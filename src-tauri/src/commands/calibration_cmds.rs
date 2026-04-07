use std::path::PathBuf;
use tauri::State;

use crate::arm::config::NUM_JOINTS;
use crate::calibration::CalibrationData;
use crate::state::AppState;

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

    // Apply offsets to both arms
    let offsets = cal.offsets;
    drop(cal);

    if let Ok(mut arm) = state.follower.lock() {
        if let Some(ref mut controller) = *arm {
            controller.set_offsets(offsets);
        }
    }

    Ok(offsets.to_vec())
}

/// Save calibration data to a file.
#[tauri::command]
pub fn save_calibration(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let cal = state.calibration.lock().map_err(|e| e.to_string())?;
    cal.save(&PathBuf::from(path))
}

/// Load calibration data from a file.
#[tauri::command]
pub fn load_calibration(state: State<'_, AppState>, path: String) -> Result<Vec<i32>, String> {
    let loaded = CalibrationData::load(&PathBuf::from(path))?;
    let offsets = loaded.offsets;

    let mut cal = state.calibration.lock().map_err(|e| e.to_string())?;
    *cal = loaded;
    drop(cal);

    // Apply offsets to follower
    if let Ok(mut arm) = state.follower.lock() {
        if let Some(ref mut controller) = *arm {
            controller.set_offsets(offsets);
        }
    }

    Ok(offsets.to_vec())
}

/// Get current calibration offsets.
#[tauri::command]
pub fn get_calibration(state: State<'_, AppState>) -> Result<Vec<i32>, String> {
    let cal = state.calibration.lock().map_err(|e| e.to_string())?;
    Ok(cal.offsets.to_vec())
}
