mod arm;
mod calibration;
mod commands;
mod recording;
mod serial;
mod state;

use commands::arm_cmds::*;
use commands::calibration_cmds::*;
use commands::mirror_cmds::*;
use commands::recording_cmds::*;
use commands::serial_cmds::*;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::new())
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
            reset_position_corrections,
            // Calibration
            calibrate_capture,
            compute_calibration,
            save_calibration,
            load_calibration,
            get_calibration,
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
