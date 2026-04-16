use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

use crate::recording::{ActiveRecording, Recording};
use crate::state::AppState;

#[tauri::command]
pub fn start_recording(
    state: State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    // Recording only makes sense while mirroring is active — that's the loop
    // appending frames into the shared recording slot.
    {
        let mt = state.mirror_thread.lock().map_err(|e| e.to_string())?;
        if mt.is_none() {
            return Err("Start mirroring before recording".to_string());
        }
    }

    let offsets = {
        let cal = state.calibration.lock().map_err(|e| e.to_string())?;
        cal.offsets
    };

    let mut rec = state.recording.lock().map_err(|e| e.to_string())?;
    if rec.is_some() {
        return Err("Already recording".to_string());
    }
    *rec = Some(ActiveRecording::new(name, offsets));
    Ok(())
}

#[tauri::command]
pub fn stop_recording(state: State<'_, AppState>) -> Result<Recording, String> {
    let mut rec = state.recording.lock().map_err(|e| e.to_string())?;
    let active = rec.take().ok_or("No active recording")?;
    Ok(active.finalize())
}

#[tauri::command]
pub fn save_recording(recording: Recording, path: String) -> Result<(), String> {
    recording.save(&PathBuf::from(path))
}

#[tauri::command]
pub fn load_recording(path: String) -> Result<Recording, String> {
    Recording::load(&PathBuf::from(path))
}

#[tauri::command]
pub fn start_playback(
    app: AppHandle,
    state: State<'_, AppState>,
    recording: Recording,
) -> Result<(), String> {
    // Refuse if mirror is running — it owns the follower controller
    {
        let mt = state.mirror_thread.lock().map_err(|e| e.to_string())?;
        if mt.is_some() {
            return Err("Stop mirroring before playback".to_string());
        }
    }
    // Refuse double-start
    {
        let pt = state.playback_thread.lock().map_err(|e| e.to_string())?;
        if pt.is_some() {
            return Err("Playback already running".to_string());
        }
    }

    let mut controller = {
        let mut follower_guard = state.follower.lock().map_err(|e| e.to_string())?;
        follower_guard.take().ok_or("Follower arm not connected")?
    };
    controller.enable_torque()?;

    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = cancel.clone();
    let frames = recording.frames.clone();
    let app_clone = app.clone();

    let handle = std::thread::spawn(move || {
        for window in frames.windows(2) {
            if cancel_clone.load(Ordering::SeqCst) {
                break;
            }
            let dt_ms = window[1].t.saturating_sub(window[0].t);
            let _ = controller.write_positions(&window[0].follower);
            std::thread::sleep(std::time::Duration::from_millis(dt_ms));
        }

        if !cancel_clone.load(Ordering::SeqCst) {
            if let Some(last) = frames.last() {
                let _ = controller.write_positions(&last.follower);
            }
        }
        let _ = controller.disable_torque();

        // Hand the follower back to AppState so the next mirror/playback can use it.
        let state = app_clone.state::<AppState>();
        if let Ok(mut f) = state.follower.lock() {
            *f = Some(controller);
        }
        // Clear our own thread/cancel slots so the UI sees playback as finished.
        if let Ok(mut h) = state.playback_thread.lock() {
            *h = None;
        }
        if let Ok(mut c) = state.playback_cancel_flag.lock() {
            *c = None;
        }

        log::info!("Playback completed");
    });

    {
        let mut h = state.playback_thread.lock().map_err(|e| e.to_string())?;
        *h = Some(handle);
    }
    {
        let mut c = state.playback_cancel_flag.lock().map_err(|e| e.to_string())?;
        *c = Some(cancel);
    }
    Ok(())
}

#[tauri::command]
pub fn stop_playback(state: State<'_, AppState>) -> Result<(), String> {
    {
        let cancel = state.playback_cancel_flag.lock().map_err(|e| e.to_string())?;
        if let Some(c) = cancel.as_ref() {
            c.store(true, Ordering::SeqCst);
        }
    }
    // Take the handle so we can join without holding the mutex; the thread itself
    // clears the slot on completion, but taking here is still safe.
    let handle = {
        let mut h = state.playback_thread.lock().map_err(|e| e.to_string())?;
        h.take()
    };
    if let Some(handle) = handle {
        let _ = handle.join();
    }
    // Make sure the cancel slot is cleared regardless of who got there first.
    {
        let mut c = state.playback_cancel_flag.lock().map_err(|e| e.to_string())?;
        *c = None;
    }
    Ok(())
}
