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
}

#[derive(Debug, Clone, Serialize)]
pub struct JointPositions {
    pub positions: [i32; NUM_JOINTS],
}

impl ArmController {
    pub fn new(bus: ServoBus, role: ArmRole) -> Self {
        Self {
            bus,
            role,
            joint_ids: DEFAULT_IDS,
            offsets: [0; NUM_JOINTS],
            last_positions: [POSITION_CENTER; NUM_JOINTS],
        }
    }

    /// Read current positions of all 6 joints.
    pub fn read_positions(&mut self) -> Result<[i32; NUM_JOINTS], String> {
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
        self.last_positions = result;
        Ok(result)
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

    /// Write goal positions to all 6 joints, applying limits.
    pub fn write_positions(&mut self, positions: &[i32; NUM_JOINTS]) -> Result<(), String> {
        let mut clamped = [0i32; NUM_JOINTS];
        for i in 0..NUM_JOINTS {
            clamped[i] = clamp_position(i, positions[i]);
        }

        let ids: Vec<u8> = self.joint_ids.to_vec();
        let pos_vec: Vec<i32> = clamped.to_vec();
        self.bus.sync_write_positions(&ids, &pos_vec)?;
        self.last_positions = clamped;
        Ok(())
    }

    /// Write a single joint position.
    pub fn write_single_joint(&mut self, joint_index: usize, position: i32) -> Result<(), String> {
        if joint_index >= NUM_JOINTS {
            return Err(format!("Joint index {} out of range", joint_index));
        }
        let clamped = clamp_position(joint_index, position);
        self.bus
            .write_position(self.joint_ids[joint_index], clamped)?;
        self.last_positions[joint_index] = clamped;
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
