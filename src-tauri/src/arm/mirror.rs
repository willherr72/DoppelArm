use std::sync::{Arc, Mutex};
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

/// Start the leader-follower mirror loop.
/// Runs on a dedicated thread (not async) because serial I/O is blocking.
/// Returns a JoinHandle that can be aborted to stop mirroring.
pub fn start_mirror_loop(
    leader: Arc<Mutex<ArmController>>,
    follower: Arc<Mutex<ArmController>>,
    calibration_offsets: [i32; NUM_JOINTS],
    recording: Arc<Mutex<Option<ActiveRecording>>>,
    channel: Channel<JointUpdatePayload>,
    cancel_rx: tokio::sync::oneshot::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let interval = Duration::from_millis(10); // 100Hz target
        let start = Instant::now();

        // Convert oneshot to a pollable mechanism
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_clone = cancel.clone();
        std::thread::spawn(move || {
            let _ = cancel_rx.blocking_recv();
            cancel_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        // Enable torque on follower, disable on leader
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
                let mut l = leader.lock().unwrap();
                l.read_positions().unwrap_or(l.last_positions)
            };

            // Apply calibration offsets and write to follower
            let mut follower_targets = [0i32; NUM_JOINTS];
            for i in 0..NUM_JOINTS {
                follower_targets[i] = leader_positions[i] + calibration_offsets[i];
            }

            let follower_actual = {
                let mut f = follower.lock().unwrap();
                let _ = f.write_positions(&follower_targets);
                f.last_positions
            };

            let timestamp_ms = start.elapsed().as_millis() as u64;

            // Emit update to frontend
            let payload = JointUpdatePayload {
                leader: leader_positions,
                follower: follower_actual,
                timestamp_ms,
            };
            let _ = channel.send(payload.clone());

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
