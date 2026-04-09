use tauri::State;

use crate::arm::config::NUM_JOINTS;
use crate::serial::bus::ServoStatus;
use crate::state::AppState;

fn get_arm_lock<'a>(
    state: &'a State<'_, AppState>,
    role: &str,
) -> Result<std::sync::MutexGuard<'a, Option<crate::arm::controller::ArmController>>, String> {
    match role {
        "leader" => state.leader.lock().map_err(|e| e.to_string()),
        "follower" => state.follower.lock().map_err(|e| e.to_string()),
        _ => Err(format!("Unknown role: {}", role)),
    }
}

#[tauri::command]
pub fn read_all_joints(state: State<'_, AppState>, role: String) -> Result<Vec<i32>, String> {
    let mut arm = get_arm_lock(&state, &role)?;
    let controller = arm.as_mut().ok_or(format!("{} arm not connected", role))?;
    let positions = controller.read_positions()?;
    Ok(positions.to_vec())
}

#[tauri::command]
pub fn write_all_joints(
    state: State<'_, AppState>,
    role: String,
    positions: Vec<i32>,
) -> Result<(), String> {
    if positions.len() != NUM_JOINTS {
        return Err(format!("Expected {} positions, got {}", NUM_JOINTS, positions.len()));
    }
    let mut arm = get_arm_lock(&state, &role)?;
    let controller = arm.as_mut().ok_or(format!("{} arm not connected", role))?;
    let mut pos_array = [0i32; NUM_JOINTS];
    pos_array.copy_from_slice(&positions);
    controller.write_positions(&pos_array)
}

#[tauri::command]
pub fn write_single_joint(
    state: State<'_, AppState>,
    role: String,
    joint_index: usize,
    position: i32,
) -> Result<(), String> {
    let mut arm = get_arm_lock(&state, &role)?;
    let controller = arm.as_mut().ok_or(format!("{} arm not connected", role))?;
    controller.write_single_joint(joint_index, position)
}

#[tauri::command]
pub fn set_torque(
    state: State<'_, AppState>,
    role: String,
    enabled: bool,
) -> Result<(), String> {
    let mut arm = get_arm_lock(&state, &role)?;
    let controller = arm.as_mut().ok_or(format!("{} arm not connected", role))?;
    if enabled {
        controller.enable_torque()
    } else {
        controller.disable_torque()
    }
}

#[tauri::command]
pub fn read_servo_status(
    state: State<'_, AppState>,
    role: String,
    servo_id: u8,
) -> Result<ServoStatus, String> {
    let mut arm = get_arm_lock(&state, &role)?;
    let controller = arm.as_mut().ok_or(format!("{} arm not connected", role))?;
    controller.bus.read_status(servo_id)
}

#[tauri::command]
pub fn enable_continuous_rotation(
    state: State<'_, AppState>,
    role: String,
    servo_id: u8,
) -> Result<(), String> {
    let mut arm = get_arm_lock(&state, &role)?;
    let controller = arm.as_mut().ok_or(format!("{} arm not connected", role))?;
    controller.bus.enable_continuous_rotation(servo_id)
}

#[tauri::command]
pub fn enable_single_turn(
    state: State<'_, AppState>,
    role: String,
    servo_id: u8,
) -> Result<(), String> {
    let mut arm = get_arm_lock(&state, &role)?;
    let controller = arm.as_mut().ok_or(format!("{} arm not connected", role))?;
    controller.bus.enable_single_turn(servo_id)
}

#[tauri::command]
pub fn set_joint_limit(
    state: State<'_, AppState>,
    role: String,
    joint_index: usize,
    min: i32,
    max: i32,
) -> Result<(), String> {
    let mut arm = get_arm_lock(&state, &role)?;
    let controller = arm.as_mut().ok_or(format!("{} arm not connected", role))?;
    controller.set_joint_limit(joint_index, min, max)
}

#[tauri::command]
pub fn get_joint_limits(
    state: State<'_, AppState>,
    role: String,
) -> Result<Vec<(i32, i32)>, String> {
    let mut arm = get_arm_lock(&state, &role)?;
    let controller = arm.as_mut().ok_or(format!("{} arm not connected", role))?;
    Ok(controller.joint_limits.to_vec())
}

/// Reset POSITION_CORRECTION on all joints to 0, undoing any prior recenter.
/// Disables torque first so the servos don't lurch.
#[tauri::command]
pub fn reset_position_corrections(
    state: State<'_, AppState>,
    role: String,
) -> Result<(), String> {
    let mut arm = get_arm_lock(&state, &role)?;
    let controller = arm.as_mut().ok_or(format!("{} arm not connected", role))?;
    let ids: Vec<u8> = controller.joint_ids.to_vec();
    for id in ids {
        controller.bus.reset_position_correction(id)?;
    }
    // Reset software unwrap state too
    controller.reset_unwrap_state();
    Ok(())
}
