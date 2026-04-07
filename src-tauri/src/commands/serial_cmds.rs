use tauri::State;

use crate::arm::config::{baud_rate_to_index, ArmRole, DEFAULT_BAUD_RATE};
use crate::arm::controller::ArmController;
use crate::serial::bus::ServoBus;
use crate::serial::port::{self, PortInfo};
use crate::state::AppState;

#[tauri::command]
pub fn list_ports() -> Vec<PortInfo> {
    port::list_serial_ports()
}

#[tauri::command]
pub fn scan_motors(port: String, baud_rate: Option<u32>) -> Result<Vec<u8>, String> {
    let baud = baud_rate.unwrap_or(DEFAULT_BAUD_RATE);
    let mut bus = ServoBus::new(&port, baud)?;
    Ok(bus.scan(1..=20))
}

#[tauri::command]
pub fn connect_arm(
    state: State<'_, AppState>,
    port: String,
    role: String,
    baud_rate: Option<u32>,
) -> Result<(), String> {
    let baud = baud_rate.unwrap_or(DEFAULT_BAUD_RATE);
    let bus = ServoBus::new(&port, baud)?;

    let arm_role = match role.as_str() {
        "leader" => ArmRole::Leader,
        "follower" => ArmRole::Follower,
        _ => return Err(format!("Unknown role: {}", role)),
    };

    let controller = ArmController::new(bus, arm_role.clone());

    match arm_role {
        ArmRole::Leader => {
            let mut leader = state.leader.lock().map_err(|e| e.to_string())?;
            *leader = Some(controller);
        }
        ArmRole::Follower => {
            let mut follower = state.follower.lock().map_err(|e| e.to_string())?;
            *follower = Some(controller);
        }
    }

    Ok(())
}

#[tauri::command]
pub fn disconnect_arm(state: State<'_, AppState>, role: String) -> Result<(), String> {
    match role.as_str() {
        "leader" => {
            let mut leader = state.leader.lock().map_err(|e| e.to_string())?;
            *leader = None;
        }
        "follower" => {
            let mut follower = state.follower.lock().map_err(|e| e.to_string())?;
            *follower = None;
        }
        _ => return Err(format!("Unknown role: {}", role)),
    }
    Ok(())
}

#[tauri::command]
pub fn configure_motor(
    port: String,
    current_id: u8,
    new_id: u8,
    baud_rate: Option<u32>,
) -> Result<(), String> {
    let baud = baud_rate.unwrap_or(DEFAULT_BAUD_RATE);
    let mut bus = ServoBus::new(&port, baud)?;

    // First try to ping the motor at current_id
    bus.ping(current_id)
        .map_err(|_| format!("No motor found at ID {}", current_id))?;

    // Set the new ID if different
    if current_id != new_id {
        bus.set_servo_id(current_id, new_id)?;
        log::info!("Motor ID changed from {} to {}", current_id, new_id);
    }

    // Set baud rate to 1Mbps (index 0) if not already
    let baud_index = baud_rate_to_index(DEFAULT_BAUD_RATE);
    bus.set_baud_rate(new_id, baud_index)?;

    Ok(())
}

/// Try to find a motor on the bus using common baud rates.
#[tauri::command]
pub fn auto_detect_motor(port: String) -> Result<(u8, u32), String> {
    let baud_rates = [1_000_000, 500_000, 115_200, 38_400];

    for &baud in &baud_rates {
        if let Ok(mut bus) = ServoBus::new(&port, baud) {
            let found = bus.scan(1..=10);
            if !found.is_empty() {
                return Ok((found[0], baud));
            }
            // Also try default ID 1
            if bus.ping(1).is_ok() {
                return Ok((1, baud));
            }
        }
    }

    Err("No motor found at any common baud rate".to_string())
}
