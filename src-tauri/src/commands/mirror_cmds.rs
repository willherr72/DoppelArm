use std::sync::atomic::{AtomicBool, Ordering};
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
        let handle = state.mirror_thread.lock().map_err(|e| e.to_string())?;
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

    {
        let mut ml = state.mirror_leader.lock().map_err(|e| e.to_string())?;
        *ml = Some(leader.clone());
    }
    {
        let mut mf = state.mirror_follower.lock().map_err(|e| e.to_string())?;
        *mf = Some(follower.clone());
    }

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut c = state.mirror_cancel_flag.lock().map_err(|e| e.to_string())?;
        *c = Some(cancel.clone());
    }

    let handle = start_mirror_loop(
        leader,
        follower,
        calibration_offsets,
        recording,
        on_update,
        cancel,
    );

    {
        let mut h = state.mirror_thread.lock().map_err(|e| e.to_string())?;
        *h = Some(handle);
    }

    Ok(())
}

#[tauri::command]
pub fn stop_mirroring(state: State<'_, AppState>) -> Result<(), String> {
    // Signal the loop to stop
    {
        let mut cancel = state.mirror_cancel_flag.lock().map_err(|e| e.to_string())?;
        if let Some(c) = cancel.take() {
            c.store(true, Ordering::SeqCst);
        }
    }

    // Join the thread
    let handle = {
        let mut h = state.mirror_thread.lock().map_err(|e| e.to_string())?;
        h.take()
    };
    if let Some(handle) = handle {
        let _ = handle.join();
    }

    // Recover controllers from Arc<Mutex> and put them back in AppState
    let leader_arc = {
        let mut ml = state.mirror_leader.lock().map_err(|e| e.to_string())?;
        ml.take()
    };
    let follower_arc = {
        let mut mf = state.mirror_follower.lock().map_err(|e| e.to_string())?;
        mf.take()
    };

    if let Some(arc) = leader_arc {
        if let Ok(mutex) = Arc::try_unwrap(arc) {
            if let Ok(controller) = mutex.into_inner() {
                let mut l = state.leader.lock().map_err(|e| e.to_string())?;
                *l = Some(controller);
            }
        }
    }
    if let Some(arc) = follower_arc {
        if let Ok(mutex) = Arc::try_unwrap(arc) {
            if let Ok(controller) = mutex.into_inner() {
                let mut f = state.follower.lock().map_err(|e| e.to_string())?;
                *f = Some(controller);
            }
        }
    }

    Ok(())
}
