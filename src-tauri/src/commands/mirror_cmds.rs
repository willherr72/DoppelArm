use std::sync::{Arc, Mutex};
use tauri::ipc::Channel;
use tauri::State;

use crate::arm::mirror::{start_mirror_loop, JointUpdatePayload};
use crate::state::AppState;

#[tauri::command]
pub fn start_mirroring(
    state: State<'_, AppState>,
    on_update: Channel<JointUpdatePayload>,
) -> Result<(), String> {
    // Check if already mirroring
    {
        let handle = state.mirror_handle.lock().map_err(|e| e.to_string())?;
        if handle.is_some() {
            return Err("Already mirroring".to_string());
        }
    }

    // Take ownership of both arm controllers for the mirror thread.
    let leader_ctrl = {
        let mut arm = state.leader.lock().map_err(|e| e.to_string())?;
        arm.take().ok_or("Leader arm not connected")?
    };
    let follower_ctrl = {
        let mut arm = state.follower.lock().map_err(|e| e.to_string())?;
        match arm.take() {
            Some(ctrl) => ctrl,
            None => {
                // Put leader back if follower not available
                let mut l = state.leader.lock().map_err(|e| e.to_string())?;
                *l = Some(leader_ctrl);
                return Err("Follower arm not connected".to_string());
            }
        }
    };

    let calibration_offsets = {
        let cal = state.calibration.lock().map_err(|e| e.to_string())?;
        cal.offsets
    };

    let recording = {
        let mut rec = state.recording.lock().map_err(|e| e.to_string())?;
        Arc::new(Mutex::new(rec.take()))
    };

    let leader = Arc::new(Mutex::new(leader_ctrl));
    let follower = Arc::new(Mutex::new(follower_ctrl));

    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();

    let handle = start_mirror_loop(
        leader.clone(),
        follower.clone(),
        calibration_offsets,
        recording.clone(),
        on_update,
        cancel_rx,
    );

    {
        let mut h = state.mirror_handle.lock().map_err(|e| e.to_string())?;
        *h = Some(handle);
    }
    {
        let mut c = state.mirror_cancel.lock().map_err(|e| e.to_string())?;
        *c = Some(cancel_tx);
    }

    Ok(())
}

#[tauri::command]
pub async fn stop_mirroring(state: State<'_, AppState>) -> Result<(), String> {
    // Send cancel signal
    {
        let mut cancel = state.mirror_cancel.lock().map_err(|e| e.to_string())?;
        if let Some(tx) = cancel.take() {
            let _ = tx.send(());
        }
    }

    // Wait for the mirror loop to finish
    let handle = {
        let mut h = state.mirror_handle.lock().map_err(|e| e.to_string())?;
        h.take()
    };
    if let Some(handle) = handle {
        let _ = handle.await;
    }

    // Note: controllers live inside Arc<Mutex> in the mirror thread.
    // After stopping, user needs to reconnect. Can be improved later
    // by recovering controllers via Arc::try_unwrap.

    Ok(())
}
