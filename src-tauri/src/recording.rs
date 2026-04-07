use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::arm::config::NUM_JOINTS;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingFrame {
    /// Milliseconds since recording start.
    pub t: u64,
    /// Leader joint positions (raw servo values).
    pub leader: [i32; NUM_JOINTS],
    /// Follower joint positions (raw servo values).
    pub follower: [i32; NUM_JOINTS],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    pub version: u32,
    pub name: String,
    pub created_at: String,
    pub duration_ms: u64,
    pub sample_rate_hz: u32,
    pub calibration_offsets: [i32; NUM_JOINTS],
    pub frames: Vec<RecordingFrame>,
}

/// Active recording state held during a recording session.
pub struct ActiveRecording {
    pub name: String,
    pub start_time_ms: u64,
    pub frames: Vec<RecordingFrame>,
    pub calibration_offsets: [i32; NUM_JOINTS],
}

impl ActiveRecording {
    pub fn new(name: String, calibration_offsets: [i32; NUM_JOINTS]) -> Self {
        Self {
            name,
            start_time_ms: 0,
            frames: Vec::with_capacity(6000), // Pre-allocate for ~60s at 100Hz
            calibration_offsets,
        }
    }

    pub fn add_frame(
        &mut self,
        leader: [i32; NUM_JOINTS],
        follower: [i32; NUM_JOINTS],
        timestamp_ms: u64,
    ) {
        if self.frames.is_empty() {
            self.start_time_ms = timestamp_ms;
        }
        self.frames.push(RecordingFrame {
            t: timestamp_ms - self.start_time_ms,
            leader,
            follower,
        });
    }

    /// Finalize the recording into a saveable format.
    pub fn finalize(self) -> Recording {
        let duration_ms = self.frames.last().map(|f| f.t).unwrap_or(0);
        let sample_rate_hz = if duration_ms > 0 && self.frames.len() > 1 {
            ((self.frames.len() as u64 - 1) * 1000 / duration_ms) as u32
        } else {
            100
        };

        Recording {
            version: 1,
            name: self.name,
            created_at: chrono::Utc::now().to_rfc3339(),
            duration_ms,
            sample_rate_hz,
            calibration_offsets: self.calibration_offsets,
            frames: self.frames,
        }
    }
}

impl Recording {
    /// Save recording to a JSON file.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize recording: {}", e))?;
        std::fs::write(path, json)
            .map_err(|e| format!("Failed to write recording file: {}", e))?;
        Ok(())
    }

    /// Load recording from a JSON file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read recording file: {}", e))?;
        let recording: Recording = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to parse recording file: {}", e))?;
        Ok(recording)
    }
}
