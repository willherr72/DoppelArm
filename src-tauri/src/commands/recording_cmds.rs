use std::path::PathBuf;
use tauri::State;

use crate::recording::{ActiveRecording, Recording};
use crate::state::AppState;

#[tauri::command]
pub fn start_recording(
    state: State<'_, AppState>,
    name: String,
) -> Result<(), String> {
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
    state: State<'_, AppState>,
    recording: Recording,
) -> Result<(), String> {
    // Check if follower is connected
    let mut follower_guard = state.follower.lock().map_err(|e| e.to_string())?;
    let follower = follower_guard
        .as_mut()
        .ok_or("Follower arm not connected")?;

    // Enable torque for playback
    follower.enable_torque()?;

    // Spawn a playback task
    let frames = recording.frames.clone();
    // We need to take the controller for the playback thread
    let mut controller = follower_guard.take().ok_or("Follower arm not connected")?;

    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();

    let handle = tokio::task::spawn_blocking(move || {
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_clone = cancel.clone();
        std::thread::spawn(move || {
            let _ = cancel_rx.blocking_recv();
            cancel_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        for window in frames.windows(2) {
            if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }

            let dt = std::time::Duration::from_millis(window[1].t - window[0].t);
            let _ = controller.write_positions(&window[0].follower);
            std::thread::sleep(dt);
        }

        // Write last frame
        if let Some(last) = frames.last() {
            let _ = controller.write_positions(&last.follower);
        }

        let _ = controller.disable_torque();
        log::info!("Playback completed");
    });

    {
        let mut h = state.playback_handle.lock().map_err(|e| e.to_string())?;
        *h = Some(handle);
    }
    {
        let mut c = state.playback_cancel.lock().map_err(|e| e.to_string())?;
        *c = Some(cancel_tx);
    }

    Ok(())
}

#[tauri::command]
pub async fn stop_playback(state: State<'_, AppState>) -> Result<(), String> {
    {
        let mut cancel = state.playback_cancel.lock().map_err(|e| e.to_string())?;
        if let Some(tx) = cancel.take() {
            let _ = tx.send(());
        }
    }
    let handle = {
        let mut h = state.playback_handle.lock().map_err(|e| e.to_string())?;
        h.take()
    };
    if let Some(handle) = handle {
        let _ = handle.await;
    }
    Ok(())
}
