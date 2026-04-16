use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

use crate::calibration::CalibrationData;
use crate::state::AppState;

/// Resolve the path where calibration data is persisted.
/// Uses Tauri's app data directory so the file lives outside the project tree
/// and won't trigger the dev watcher's rebuild loop.
pub(crate) fn calibration_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {}", e))?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create app data dir: {}", e))?;
    Ok(dir.join("calibration.json"))
}

/// Capture the current position of all joints on the specified arm as a calibration reference.
#[tauri::command]
pub fn calibrate_capture(
    state: State<'_, AppState>,
    role: String,
) -> Result<Vec<i32>, String> {
    match role.as_str() {
        "leader" => {
            let mut arm = state.leader.lock().map_err(|e| e.to_string())?;
            let controller = arm.as_mut().ok_or("Leader arm not connected")?;
            let positions = controller.read_positions()?;

            let mut cal = state.calibration.lock().map_err(|e| e.to_string())?;
            cal.leader_reference = positions;
            Ok(positions.to_vec())
        }
        "follower" => {
            let mut arm = state.follower.lock().map_err(|e| e.to_string())?;
            let controller = arm.as_mut().ok_or("Follower arm not connected")?;
            let positions = controller.read_positions()?;

            let mut cal = state.calibration.lock().map_err(|e| e.to_string())?;
            cal.follower_reference = positions;
            Ok(positions.to_vec())
        }
        _ => Err(format!("Unknown role: {}", role)),
    }
}

/// Compute calibration offsets from previously captured reference positions.
#[tauri::command]
pub fn compute_calibration(state: State<'_, AppState>) -> Result<Vec<i32>, String> {
    let mut cal = state.calibration.lock().map_err(|e| e.to_string())?;
    cal.compute_offsets();

    let offsets = cal.offsets;
    drop(cal);

    if let Ok(mut arm) = state.follower.lock() {
        if let Some(ref mut controller) = *arm {
            controller.set_offsets(offsets);
        }
    }

    Ok(offsets.to_vec())
}

/// Save calibration data to the app data directory.
#[tauri::command]
pub fn save_calibration(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let path = calibration_path(&app)?;
    let cal = state.calibration.lock().map_err(|e| e.to_string())?;
    cal.save(&path)
}

/// Load calibration data from the app data directory.
#[tauri::command]
pub fn load_calibration(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<i32>, String> {
    let path = calibration_path(&app)?;
    let loaded = CalibrationData::load(&path)?;
    let offsets = loaded.offsets;

    let mut cal = state.calibration.lock().map_err(|e| e.to_string())?;
    *cal = loaded;
    drop(cal);

    if let Ok(mut arm) = state.follower.lock() {
        if let Some(ref mut controller) = *arm {
            controller.set_offsets(offsets);
        }
    }

    Ok(offsets.to_vec())
}

#[tauri::command]
pub fn has_saved_calibration(app: AppHandle) -> Result<bool, String> {
    let path = calibration_path(&app)?;
    Ok(path.exists())
}

#[tauri::command]
pub fn get_calibration_state(state: State<'_, AppState>) -> Result<CalibrationData, String> {
    let cal = state.calibration.lock().map_err(|e| e.to_string())?;
    Ok(cal.clone())
}

#[tauri::command]
pub fn set_calibration_state(
    state: State<'_, AppState>,
    calibration: CalibrationData,
) -> Result<(), String> {
    let offsets = calibration.offsets;

    let mut cal = state.calibration.lock().map_err(|e| e.to_string())?;
    *cal = calibration;
    drop(cal);

    if let Ok(mut arm) = state.follower.lock() {
        if let Some(ref mut controller) = *arm {
            controller.set_offsets(offsets);
        }
    }

    Ok(())
}

/// Get current calibration offsets.
#[tauri::command]
pub fn get_calibration(state: State<'_, AppState>) -> Result<Vec<i32>, String> {
    let cal = state.calibration.lock().map_err(|e| e.to_string())?;
    Ok(cal.offsets.to_vec())
}

/// Get current per-joint mirror direction signs.
#[tauri::command]
pub fn get_mirror_signs(state: State<'_, AppState>) -> Result<Vec<i32>, String> {
    let cal = state.calibration.lock().map_err(|e| e.to_string())?;
    Ok(cal.mirror_signs.to_vec())
}

/// Set one joint's mirror direction sign to either 1 or -1.
#[tauri::command]
pub fn set_mirror_sign(
    state: State<'_, AppState>,
    joint_index: usize,
    sign: i32,
) -> Result<Vec<i32>, String> {
    if joint_index >= crate::arm::config::NUM_JOINTS {
        return Err(format!("Joint index {} out of range", joint_index));
    }
    if sign != 1 && sign != -1 {
        return Err(format!("Invalid sign {}. Expected 1 or -1", sign));
    }

    let mut cal = state.calibration.lock().map_err(|e| e.to_string())?;
    cal.mirror_signs[joint_index] = sign;
    cal.compute_offsets();
    let mirror_signs = cal.mirror_signs;
    let offsets = cal.offsets;
    drop(cal);

    if let Ok(mut arm) = state.follower.lock() {
        if let Some(ref mut controller) = *arm {
            controller.set_offsets(offsets);
        }
    }

    Ok(mirror_signs.to_vec())
}
