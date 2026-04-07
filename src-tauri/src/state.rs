use std::sync::Mutex;
use tokio::task::JoinHandle;

use crate::arm::controller::ArmController;
use crate::calibration::CalibrationData;
use crate::recording::ActiveRecording;

pub struct AppState {
    pub leader: Mutex<Option<ArmController>>,
    pub follower: Mutex<Option<ArmController>>,
    pub mirror_handle: Mutex<Option<JoinHandle<()>>>,
    pub mirror_cancel: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    pub playback_handle: Mutex<Option<JoinHandle<()>>>,
    pub playback_cancel: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    pub recording: Mutex<Option<ActiveRecording>>,
    pub calibration: Mutex<CalibrationData>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            leader: Mutex::new(None),
            follower: Mutex::new(None),
            mirror_handle: Mutex::new(None),
            mirror_cancel: Mutex::new(None),
            playback_handle: Mutex::new(None),
            playback_cancel: Mutex::new(None),
            recording: Mutex::new(None),
            calibration: Mutex::new(CalibrationData::default()),
        }
    }
}
