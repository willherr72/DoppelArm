use serde::Serialize;

use crate::arm::config::*;
use crate::serial::bus::ServoBus;

/// High-level controller for a single 6-DOF arm.
pub struct ArmController {
    pub bus: ServoBus,
    pub role: ArmRole,
    pub joint_ids: [u8; NUM_JOINTS],
    pub offsets: [i32; NUM_JOINTS],
    pub last_positions: [i32; NUM_JOINTS],
    /// Per-joint position limits for software clamping. Defaults to 0-4095.
    pub joint_limits: [(i32, i32); NUM_JOINTS],
    joint_wraps: [bool; NUM_JOINTS],
    /// Software unwrap state — last raw value seen and accumulated wrap count
    /// per joint. Used to compute continuous extended positions across the
    /// 0/4095 boundary.
    last_raw: [Option<i32>; NUM_JOINTS],
    wrap_count: [i32; NUM_JOINTS],
}

#[derive(Debug, Clone, Serialize)]
pub struct JointPositions {
    pub positions: [i32; NUM_JOINTS],
}

impl ArmController {
    pub fn new(bus: ServoBus, role: ArmRole) -> Self {
        let joint_ids = ids_for_role(&role);
        let mut joint_limits = default_limits_for_role(&role);
        let mut joint_wraps = [false; NUM_JOINTS];
        if matches!(role, ArmRole::Follower) {
            joint_limits[5] = (2958, 4095);
            for joint_index in WRAPPED_MIRROR_JOINTS {
                joint_wraps[joint_index] = true;
            }
        }
        Self {
            bus,
            role,
            joint_ids,
            offsets: [0; NUM_JOINTS],
            last_positions: [POSITION_CENTER; NUM_JOINTS],
            joint_limits,
            joint_wraps,
            last_raw: [None; NUM_JOINTS],
            wrap_count: [0; NUM_JOINTS],
        }
    }

    /// Read raw single-turn positions of all joints (0-4095, no unwrap).
    pub fn read_raw_positions(&mut self) -> Result<[i32; NUM_JOINTS], String> {
        let ids: Vec<u8> = self.joint_ids.to_vec();
        let positions = self.bus.sync_read_positions(&ids)?;
        if positions.len() != NUM_JOINTS {
            return Err(format!(
                "Expected {} positions, got {}",
                NUM_JOINTS,
                positions.len()
            ));
        }
        let mut result = [0i32; NUM_JOINTS];
        for i in 0..NUM_JOINTS {
            result[i] = positions[i];
        }
        Ok(result)
    }

    /// Read positions and apply software unwrap. The first read seeds the
    /// state and returns raw values directly. Subsequent reads detect wraps
    /// across the 0/4095 boundary and return continuous extended positions.
    pub fn read_positions(&mut self) -> Result<[i32; NUM_JOINTS], String> {
        let raw = self.read_raw_positions()?;
        let mut unwrapped = [0i32; NUM_JOINTS];
        for i in 0..NUM_JOINTS {
            if let Some(prev) = self.last_raw[i] {
                let delta = raw[i] - prev;
                // If delta is more than half a turn, we wrapped.
                // delta > 2048: wrapped backward (e.g. went from 5 to 4090)
                // delta < -2048: wrapped forward (e.g. went from 4090 to 5)
                if delta > 2048 {
                    self.wrap_count[i] -= 1;
                } else if delta < -2048 {
                    self.wrap_count[i] += 1;
                }
            }
            self.last_raw[i] = Some(raw[i]);
            unwrapped[i] = raw[i] + self.wrap_count[i] * 4096;
        }
        self.last_positions = unwrapped;
        Ok(unwrapped)
    }

    /// Reset the software unwrap state. Call after recentering or when the
    /// servo's reference frame has changed.
    pub fn reset_unwrap_state(&mut self) {
        self.last_raw = [None; NUM_JOINTS];
        self.wrap_count = [0; NUM_JOINTS];
    }

    /// Read positions with calibration offsets applied.
    pub fn read_calibrated_positions(&mut self) -> Result<[i32; NUM_JOINTS], String> {
        let raw = self.read_positions()?;
        let mut calibrated = [0i32; NUM_JOINTS];
        for i in 0..NUM_JOINTS {
            calibrated[i] = raw[i] + self.offsets[i];
        }
        Ok(calibrated)
    }

    /// Clamp a position to this arm's per-joint limits.
    fn clamp(&self, joint_index: usize, position: i32) -> i32 {
        let (min, max) = self.joint_limits[joint_index];
        position.clamp(min, max)
    }

    fn nearest_wrapped_target(&self, joint_index: usize, position: i32) -> i32 {
        let wrapped = position.rem_euclid(4096);
        let last = self.last_positions[joint_index];
        let turns = (last - wrapped) as f64 / 4096.0;
        wrapped + (turns.round() as i32) * 4096
    }

    fn normalize_target(&self, joint_index: usize, extended: i32) -> (i32, i32) {
        if self.joint_wraps[joint_index] {
            let nearest = self.nearest_wrapped_target(joint_index, extended);
            let wrapped = nearest.rem_euclid(4096);
            (nearest, wrapped)
        } else {
            let clamped = self.clamp(joint_index, extended);
            (clamped, clamped.clamp(0, 4095))
        }
    }

    /// Write goal positions to all 6 joints. Accepts extended positions
    /// (e.g. from leader unwrap) and converts to single-turn raw values.
    /// Per-arm limits are applied to the EXTENDED value before conversion.
    pub fn write_positions(&mut self, positions: &[i32; NUM_JOINTS]) -> Result<(), String> {
        let mut to_send = [0i32; NUM_JOINTS];
        let mut normalized = [0i32; NUM_JOINTS];
        for i in 0..NUM_JOINTS {
            let (extended, raw) = self.normalize_target(i, positions[i]);
            normalized[i] = extended;
            to_send[i] = raw;
        }

        let ids: Vec<u8> = self.joint_ids.to_vec();
        let pos_vec: Vec<i32> = to_send.to_vec();
        self.bus.sync_write_positions(&ids, &pos_vec)?;
        self.last_positions = normalized;
        Ok(())
    }

    /// Write a single joint position. Accepts extended values; converts to raw.
    pub fn write_single_joint(&mut self, joint_index: usize, position: i32) -> Result<(), String> {
        if joint_index >= NUM_JOINTS {
            return Err(format!("Joint index {} out of range", joint_index));
        }
        let (extended, raw) = self.normalize_target(joint_index, position);
        self.bus.write_position(self.joint_ids[joint_index], raw)?;
        self.last_positions[joint_index] = extended;
        Ok(())
    }

    pub fn set_joint_limit(&mut self, joint_index: usize, min: i32, max: i32) -> Result<(), String> {
        if joint_index >= NUM_JOINTS {
            return Err(format!("Joint index {} out of range", joint_index));
        }
        self.joint_limits[joint_index] = (min, max);
        Ok(())
    }

    /// Enable torque on all joints (follower mode - servos hold position).
    pub fn enable_torque(&mut self) -> Result<(), String> {
        let ids: Vec<u8> = self.joint_ids.to_vec();
        self.bus.sync_set_torque(&ids, true)
    }

    /// Disable torque on all joints (leader mode - servos are free to move).
    pub fn disable_torque(&mut self) -> Result<(), String> {
        let ids: Vec<u8> = self.joint_ids.to_vec();
        self.bus.sync_set_torque(&ids, false)
    }

    /// Set calibration offsets.
    pub fn set_offsets(&mut self, offsets: [i32; NUM_JOINTS]) {
        self.offsets = offsets;
    }

    /// Get the last known positions (cached from the most recent read).
    pub fn get_last_positions(&self) -> [i32; NUM_JOINTS] {
        self.last_positions
    }
}
