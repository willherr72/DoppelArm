use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::arm::controller::ArmController;
use crate::calibration::CalibrationData;
use crate::recording::ActiveRecording;

pub struct AppState {
    pub leader: Mutex<Option<ArmController>>,
    pub follower: Mutex<Option<ArmController>>,
    /// Mirror loop thread handle
    pub mirror_thread: Mutex<Option<JoinHandle<()>>>,
    /// Cancel flag for the mirror loop
    pub mirror_cancel_flag: Mutex<Option<Arc<AtomicBool>>>,
    /// Holds Arc refs to controllers while mirroring, so we can recover them after.
    pub mirror_leader: Mutex<Option<Arc<Mutex<ArmController>>>>,
    pub mirror_follower: Mutex<Option<Arc<Mutex<ArmController>>>>,
    pub playback_thread: Mutex<Option<JoinHandle<()>>>,
    pub playback_cancel_flag: Mutex<Option<Arc<AtomicBool>>>,
    pub recording: Mutex<Option<ActiveRecording>>,
    pub calibration: Mutex<CalibrationData>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            leader: Mutex::new(None),
            follower: Mutex::new(None),
            mirror_thread: Mutex::new(None),
            mirror_cancel_flag: Mutex::new(None),
            mirror_leader: Mutex::new(None),
            mirror_follower: Mutex::new(None),
            playback_thread: Mutex::new(None),
            playback_cancel_flag: Mutex::new(None),
            recording: Mutex::new(None),
            calibration: Mutex::new(CalibrationData::default()),
        }
    }
}
