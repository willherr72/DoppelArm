use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::ipc::Channel;

use crate::arm::config::NUM_JOINTS;
use crate::arm::controller::ArmController;
use crate::recording::ActiveRecording;

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
    recording: Arc<Mutex<Option<ActiveRecording>>>,
    channel: Channel<JointUpdatePayload>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let interval = Duration::from_millis(20); // ~50Hz target
        let start = Instant::now();

        // Enable torque on follower, leave leader free to move
        {
            if let Ok(mut f) = follower.lock() {
                let _ = f.enable_torque();
            }
            if let Ok(mut l) = leader.lock() {
                let _ = l.disable_torque();
            }
        }

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

            // Apply calibration offsets and write to follower
            let mut follower_targets = [0i32; NUM_JOINTS];
            for i in 0..NUM_JOINTS {
                follower_targets[i] = leader_positions[i] + calibration_offsets[i];
            }

            let follower_actual = {
                let mut f = match follower.lock() {
                    Ok(g) => g,
                    Err(_) => break,
                };
                let _ = f.write_positions(&follower_targets);
                f.last_positions
            };

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
