use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::ipc::Channel;

use crate::arm::config::{NUM_JOINTS, WRAPPED_MIRROR_JOINTS};
use crate::arm::controller::ArmController;
use crate::recording::ActiveRecording;

// Mirror tuning constants. All values are in raw servo units, not degrees.
// Tune in this order:
// 1. Increase DEADband or SEND threshold to suppress micro-twitching.
// 2. Lower FILTER_ALPHA for more smoothing, raise it for more responsiveness.
// 3. Lower MAX_STEP_PER_CYCLE to soften sharp single-cycle jumps.
// Joint 0 (shoulder_pan) is intentionally tuned for slow-motion continuity:
// lower deadbands, higher alpha, and a smaller max step reduce the
// "hold, then jump" behavior that shows up during very slow panning.
const MIRROR_DEBUG_LOGGING: bool = false;
const LEADER_DEADBAND_RAW: [i32; NUM_JOINTS] = [1, 3, 3, 3, 3, 2];
const SEND_DEADBAND_RAW: [i32; NUM_JOINTS] = [1, 4, 4, 4, 4, 3];
const FILTER_ALPHA: [f32; NUM_JOINTS] = [0.55, 0.35, 0.35, 0.35, 0.35, 0.45];
const MAX_STEP_PER_CYCLE_RAW: [i32; NUM_JOINTS] = [10, 24, 24, 24, 24, 32];

#[derive(Debug, Clone, Serialize)]
pub struct JointUpdatePayload {
    pub leader: [i32; NUM_JOINTS],
    pub follower: [i32; NUM_JOINTS],
    pub timestamp_ms: u64,
}

/// Start the leader-follower mirror loop on a dedicated OS thread.
/// Returns a thread JoinHandle that can be joined after the cancel
/// signal has been sent.
pub fn start_mirror_loop(
    leader: Arc<Mutex<ArmController>>,
    follower: Arc<Mutex<ArmController>>,
    calibration_offsets: [i32; NUM_JOINTS],
    mirror_signs: [i32; NUM_JOINTS],
    leader_reference: [i32; NUM_JOINTS],
    follower_reference: [i32; NUM_JOINTS],
    recording: Arc<Mutex<Option<ActiveRecording>>>,
    channel: Channel<JointUpdatePayload>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let interval = Duration::from_millis(20); // ~50Hz target
        let start = Instant::now();

        // Enable torque on follower, leave leader free to move
        let leader_seed = {
            let mut seed = [0i32; NUM_JOINTS];
            if let Ok(mut l) = leader.lock() {
                let _ = l.disable_torque();
                seed = l.read_positions().unwrap_or(l.last_positions);
            }
            seed
        };
        let follower_seed = {
            let mut seed = [0i32; NUM_JOINTS];
            if let Ok(mut f) = follower.lock() {
                let _ = f.enable_torque();
                seed = f.read_positions().unwrap_or(f.last_positions);
                for joint_index in WRAPPED_MIRROR_JOINTS {
                    let target = wrapped_follower_target(
                        joint_index,
                        leader_seed,
                        mirror_signs,
                        leader_reference,
                        follower_reference,
                    );
                    seed[joint_index] =
                        align_wrapped_position(seed[joint_index], target);
                }
                f.last_positions = seed;
            }
            seed
        };

        if leader_seed == [0; NUM_JOINTS] || follower_seed == [0; NUM_JOINTS] {
            log::warn!("Mirror startup could not fully seed wrapped joint state");
        }

        let mut filtered_leader = leader_seed;
        let mut last_sent_target = follower_seed;

        loop {
            if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }

            let loop_start = Instant::now();

            // Read leader positions
            let leader_positions = {
                let mut l = match leader.lock() {
                    Ok(g) => g,
                    Err(_) => break,
                };
                l.read_positions().unwrap_or(l.last_positions)
            };
            let filtered_positions = filter_leader_positions(leader_positions, &mut filtered_leader);

            // Apply calibration offsets and write to follower
            let mut follower_targets = [0i32; NUM_JOINTS];
            for i in 0..NUM_JOINTS {
                follower_targets[i] = if WRAPPED_MIRROR_JOINTS.contains(&i) {
                    wrapped_follower_target(
                        i,
                        filtered_positions,
                        mirror_signs,
                        leader_reference,
                        follower_reference,
                    )
                } else {
                    filtered_positions[i] * mirror_signs[i] + calibration_offsets[i]
                };
            }
            follower_targets = rate_limit_targets(follower_targets, last_sent_target);

            let should_send = should_send_targets(follower_targets, last_sent_target);

            let follower_actual = {
                let mut f = match follower.lock() {
                    Ok(g) => g,
                    Err(_) => break,
                };
                if should_send {
                    let _ = f.write_positions(&follower_targets);
                    last_sent_target = f.last_positions;
                }
                f.last_positions
            };

            if MIRROR_DEBUG_LOGGING {
                log::debug!(
                    "mirror raw={:?} filtered={:?} last_sent={:?} should_send={}",
                    leader_positions,
                    filtered_positions,
                    last_sent_target,
                    should_send
                );
            }
            if !should_send && MIRROR_DEBUG_LOGGING {
                log::debug!("mirror skipped follower write due to send deadband");
            }

            let timestamp_ms = start.elapsed().as_millis() as u64;

            // Emit update to frontend (catch any send error gracefully)
            let payload = JointUpdatePayload {
                leader: leader_positions,
                follower: follower_actual,
                timestamp_ms,
            };
            // Wrap channel send in catch_unwind so a panic in the FFI
            // boundary cannot crash the whole process.
            let send_result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| channel.send(payload)));
            if send_result.is_err() {
                log::error!("Channel send panicked, stopping mirror loop");
                break;
            }

            // Record frame if recording is active
            if let Ok(mut rec) = recording.lock() {
                if let Some(ref mut active) = *rec {
                    active.add_frame(leader_positions, follower_actual, timestamp_ms);
                }
            }

            // Maintain target loop rate
            let elapsed = loop_start.elapsed();
            if elapsed < interval {
                std::thread::sleep(interval - elapsed);
            }
        }

        // Disable torque on follower when done
        if let Ok(mut f) = follower.lock() {
            let _ = f.disable_torque();
        }

        log::info!("Mirror loop stopped");
    })
}

fn align_wrapped_position(raw_position: i32, target: i32) -> i32 {
    let wrapped = raw_position.rem_euclid(4096);
    let turns = ((target - wrapped) as f64 / 4096.0).round() as i32;
    wrapped + turns * 4096
}

fn filter_leader_positions(
    raw_positions: [i32; NUM_JOINTS],
    filtered_positions: &mut [i32; NUM_JOINTS],
) -> [i32; NUM_JOINTS] {
    for i in 0..NUM_JOINTS {
        let raw = raw_positions[i];
        let previous = filtered_positions[i];
        let delta = raw - previous;
        if delta.abs() <= LEADER_DEADBAND_RAW[i] {
            continue;
        }

        let alpha = FILTER_ALPHA[i] as f64;
        let smoothed = previous as f64 + (raw - previous) as f64 * alpha;
        filtered_positions[i] = smoothed.round() as i32;
    }

    *filtered_positions
}

fn should_send_targets(
    new_targets: [i32; NUM_JOINTS],
    last_sent_targets: [i32; NUM_JOINTS],
) -> bool {
    (0..NUM_JOINTS).any(|i| {
        circular_distance(new_targets[i], last_sent_targets[i], WRAPPED_MIRROR_JOINTS.contains(&i))
            > SEND_DEADBAND_RAW[i]
    })
}

fn rate_limit_targets(
    targets: [i32; NUM_JOINTS],
    last_sent_targets: [i32; NUM_JOINTS],
) -> [i32; NUM_JOINTS] {
    let mut limited = targets;
    for i in 0..NUM_JOINTS {
        let max_step = MAX_STEP_PER_CYCLE_RAW[i];
        if max_step <= 0 {
            continue;
        }

        let wrapped = WRAPPED_MIRROR_JOINTS.contains(&i);
        let delta = circular_delta(limited[i], last_sent_targets[i], wrapped);
        let clamped_delta = delta.clamp(-max_step, max_step);
        limited[i] = last_sent_targets[i] + clamped_delta;
    }
    limited
}

fn circular_distance(current: i32, previous: i32, wrapped: bool) -> i32 {
    if wrapped {
        circular_delta(current, previous, true).abs()
    } else {
        (current - previous).abs()
    }
}

fn circular_delta(current: i32, previous: i32, wrapped: bool) -> i32 {
    if wrapped {
        wrap_delta(current, previous)
    } else {
        current - previous
    }
}

fn wrapped_follower_target(
    joint_index: usize,
    leader_positions: [i32; NUM_JOINTS],
    mirror_signs: [i32; NUM_JOINTS],
    leader_reference: [i32; NUM_JOINTS],
    follower_reference: [i32; NUM_JOINTS],
) -> i32 {
    // Measure leader motion relative to the captured calibration pose using the
    // shortest circular delta so seam crossings stay continuous.
    let signed_leader = leader_positions[joint_index] * mirror_signs[joint_index];
    let signed_reference = leader_reference[joint_index] * mirror_signs[joint_index];
    let leader_delta = wrap_delta(signed_leader, signed_reference);

    wrap_position(follower_reference[joint_index] + leader_delta)
}

fn wrap_delta(current: i32, reference: i32) -> i32 {
    let mut delta = (current - reference).rem_euclid(4096);
    if delta > 2048 {
        delta -= 4096;
    }
    delta
}

fn wrap_position(position: i32) -> i32 {
    position.rem_euclid(4096)
}
