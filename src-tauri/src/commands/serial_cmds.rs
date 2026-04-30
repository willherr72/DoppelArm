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
pub async fn scan_motors(port: String, baud_rate: Option<u32>) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        let baud = baud_rate.unwrap_or(DEFAULT_BAUD_RATE);
        let mut bus = ServoBus::new(&port, baud)?;
        // Scan only IDs 1-10 (more than enough for 6-motor arms)
        Ok(bus.scan(1..=10))
    })
    .await
    .map_err(|e| format!("scan task failed: {}", e))?
}

/// Scan for motor IDs using the already-connected bus of the given arm.
/// Use this when the arm is connected, since opening the port again would fail.
#[tauri::command]
pub fn scan_connected(state: State<'_, AppState>, role: String) -> Result<Vec<u8>, String> {
    match role.as_str() {
        "leader" => {
            let mut arm = state.leader.lock().map_err(|e| e.to_string())?;
            let controller = arm.as_mut().ok_or("Leader arm not connected")?;
            Ok(controller.bus.scan(1..=10))
        }
        "follower" => {
            let mut arm = state.follower.lock().map_err(|e| e.to_string())?;
            let controller = arm.as_mut().ok_or("Follower arm not connected")?;
            Ok(controller.bus.scan(1..=10))
        }
        _ => Err(format!("Unknown role: {}", role)),
    }
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
            let offsets = {
                let cal = state.calibration.lock().map_err(|e| e.to_string())?;
                cal.offsets
            };

            let mut follower = state.follower.lock().map_err(|e| e.to_string())?;
            let mut controller = controller;
            controller.set_offsets(offsets);
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
/// Scans IDs 1-10 at each baud; returns the first responding (id, baud).
#[tauri::command]
pub fn auto_detect_motor(port: String) -> Result<(u8, u32), String> {
    let baud_rates = [1_000_000, 500_000, 250_000, 115_200, 38_400];

    for &baud in &baud_rates {
        if let Ok(mut bus) = ServoBus::new(&port, baud) {
            let found = bus.scan(1..=10);
            if let Some(&id) = found.first() {
                return Ok((id, baud));
            }
        }
    }

    Err("No motor found at any common baud rate".to_string())
}

/// Diagnostic: scan IDs 1-10 at each common baud rate and report findings.
#[tauri::command]
pub async fn diagnose_port(port: String) -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(move || {
        let baud_rates: Vec<u32> = vec![1_000_000, 500_000, 250_000, 115_200, 38_400];
        let mut results = Vec::new();

        for baud in baud_rates {
            let msg = match ServoBus::new(&port, baud) {
                Ok(mut bus) => {
                    let found = bus.scan(1..=10);
                    if found.is_empty() {
                        format!("{}: no motors found", baud)
                    } else {
                        format!("{}: OK - found IDs {}", baud, format_id_list(&found))
                    }
                }
                Err(e) => format!("{}: failed to open - {}", baud, e),
            };
            results.push(msg);
        }

        Ok::<_, String>(results)
    })
    .await
    .map_err(|e| format!("diagnose task failed: {}", e))?
}

fn format_id_list(ids: &[u8]) -> String {
    ids.iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}
