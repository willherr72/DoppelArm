mod arm;
mod calibration;
mod commands;
mod recording;
mod serial;
mod state;

use calibration::CalibrationData;
use commands::arm_cmds::*;
use commands::calibration_cmds::*;
use commands::mirror_cmds::*;
use commands::recording_cmds::*;
use commands::serial_cmds::*;
use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::new())
        .setup(|app| {
            let path = calibration_path(&app.handle())?;
            if path.exists() {
                match CalibrationData::load(&path) {
                    Ok(loaded) => {
                        let state = app.state::<AppState>();
                        let mut cal = state
                            .calibration
                            .lock()
                            .map_err(|e: std::sync::PoisonError<_>| e.to_string())?;
                        *cal = loaded;
                        log::info!("Loaded saved calibration from {}", path.display());
                    }
                    Err(error) => {
                        log::warn!("Failed to auto-load saved calibration: {}", error);
                    }
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Serial / connection
            list_ports,
            scan_motors,
            scan_connected,
            connect_arm,
            disconnect_arm,
            configure_motor,
            auto_detect_motor,
            diagnose_port,
            // Arm control
            read_all_joints,
            write_all_joints,
            write_single_joint,
            set_torque,
            read_servo_status,
            enable_continuous_rotation,
            enable_single_turn,
            set_joint_limit,
            get_joint_limits,
            // Calibration
            calibrate_capture,
            compute_calibration,
            save_calibration,
            load_calibration,
            has_saved_calibration,
            get_calibration_state,
            set_calibration_state,
            get_calibration,
            get_mirror_signs,
            set_mirror_sign,
            // Mirroring
            start_mirroring,
            stop_mirroring,
            // Recording
            start_recording,
            stop_recording,
            save_recording,
            load_recording,
            start_playback,
            stop_playback,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
