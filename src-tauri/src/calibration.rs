use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::arm::config::NUM_JOINTS;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationData {
    /// Per-joint offsets: follower_target = leader_position + offset[i]
    pub offsets: [i32; NUM_JOINTS],
    /// Raw positions captured from the leader arm at the reference pose.
    pub leader_reference: [i32; NUM_JOINTS],
    /// Raw positions captured from the follower arm at the reference pose.
    pub follower_reference: [i32; NUM_JOINTS],
    /// Serial port used for the leader arm.
    pub leader_port: String,
    /// Serial port used for the follower arm.
    pub follower_port: String,
}

impl Default for CalibrationData {
    fn default() -> Self {
        Self {
            offsets: [0; NUM_JOINTS],
            leader_reference: [2048; NUM_JOINTS],
            follower_reference: [2048; NUM_JOINTS],
            leader_port: String::new(),
            follower_port: String::new(),
        }
    }
}

impl CalibrationData {
    /// Compute offsets from captured reference positions.
    /// Both arms should be physically placed in the same reference pose.
    /// offset[i] = follower_reference[i] - leader_reference[i]
    pub fn compute_offsets(&mut self) {
        for i in 0..NUM_JOINTS {
            self.offsets[i] = self.follower_reference[i] - self.leader_reference[i];
        }
    }

    /// Save calibration data to a JSON file.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize calibration: {}", e))?;
        std::fs::write(path, json)
            .map_err(|e| format!("Failed to write calibration file: {}", e))?;
        Ok(())
    }

    /// Load calibration data from a JSON file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read calibration file: {}", e))?;
        let data: CalibrationData = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to parse calibration file: {}", e))?;
        Ok(data)
    }
}
