use serde::{Deserialize, Serialize};

pub const NUM_JOINTS: usize = 6;
pub const DEFAULT_BAUD_RATE: u32 = 1_000_000;

/// Standard joint -> servo ID mapping (kinematic index = servo ID).
pub const STANDARD_IDS: [u8; NUM_JOINTS] = [1, 2, 3, 4, 5, 6];

/// Leader-arm mapping with wrist_roll and gripper swapped.
/// The user's leader has the gripper motor at ID 5 and wrist roll motor at ID 6.
pub const LEADER_IDS: [u8; NUM_JOINTS] = [1, 2, 3, 4, 6, 5];

/// Default servo IDs (kept for backwards compatibility).
pub const DEFAULT_IDS: [u8; NUM_JOINTS] = STANDARD_IDS;

/// Get the joint -> servo ID mapping for a given arm role.
pub fn ids_for_role(role: &ArmRole) -> [u8; NUM_JOINTS] {
    match role {
        ArmRole::Leader => LEADER_IDS,
        ArmRole::Follower => STANDARD_IDS,
    }
}

/// Default per-joint position limits per arm role.
/// Leader is unrestricted (gears removed, free movement).
/// Follower has the gripper clamped to a safe physical range
/// (3208-4095 raw ≈ 282°-360°, the closed/open envelope).
pub fn default_limits_for_role(role: &ArmRole) -> [(i32, i32); NUM_JOINTS] {
    match role {
        ArmRole::Leader => [(0, 4095); NUM_JOINTS],
        ArmRole::Follower => [
            (0, 4095),     // shoulder_pan
            (0, 4095),     // shoulder_lift
            (0, 4095),     // elbow_flex
            (0, 4095),     // wrist_flex
            (0, 4095),     // wrist_roll
            (3208, 4095),  // gripper: ~282° (closed) to 360° (open)
        ],
    }
}

/// Joint names in order.
pub const JOINT_NAMES: [&str; NUM_JOINTS] = [
    "shoulder_pan",
    "shoulder_lift",
    "elbow_flex",
    "wrist_flex",
    "wrist_roll",
    "gripper",
];

/// Position range for STS3215: 0-4095 maps to 0-360 degrees.
pub const POSITION_MIN: i32 = 0;
pub const POSITION_MAX: i32 = 4095;
pub const POSITION_CENTER: i32 = 2048;

/// Per-joint position limits (raw servo units).
/// We use the full 0-4095 range and let the hardware enforce its own
/// mechanical limits. Software clamping was masking valid leader motions.
pub const JOINT_LIMITS: [(i32, i32); NUM_JOINTS] = [
    (0, 4095),  // shoulder_pan
    (0, 4095),  // shoulder_lift
    (0, 4095),  // elbow_flex
    (0, 4095),  // wrist_flex
    (0, 4095),  // wrist_roll
    (0, 4095),  // gripper
];

/// Link lengths in millimeters for FK/visualization.
pub const LINK_BASE_HEIGHT: f64 = 55.0;
pub const LINK_UPPER_ARM: f64 = 104.0;
pub const LINK_FOREARM: f64 = 95.0;
pub const LINK_WRIST_TO_EE: f64 = 70.0;

/// Baud rate index values for the STS3215 EEPROM.
/// Write this index to register 6 to set the baud rate.
pub fn baud_rate_to_index(baud: u32) -> u8 {
    match baud {
        1_000_000 => 0,
        500_000 => 1,
        250_000 => 2,
        128_000 => 3,
        115_200 => 4,
        76_800 => 5,
        57_600 => 6,
        38_400 => 7,
        _ => 0, // Default to 1Mbps
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArmRole {
    Leader,
    Follower,
}

impl std::fmt::Display for ArmRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArmRole::Leader => write!(f, "Leader"),
            ArmRole::Follower => write!(f, "Follower"),
        }
    }
}

/// Clamp a position to the limits for a given joint index.
pub fn clamp_position(joint_index: usize, position: i32) -> i32 {
    let (min, max) = JOINT_LIMITS[joint_index];
    position.clamp(min, max)
}

/// Convert raw servo position (0-4095) to radians (0-2*PI).
pub fn raw_to_radians(raw: i32) -> f64 {
    (raw as f64 / 4095.0) * 2.0 * std::f64::consts::PI
}

/// Convert radians to raw servo position.
pub fn radians_to_raw(radians: f64) -> i32 {
    ((radians / (2.0 * std::f64::consts::PI)) * 4095.0) as i32
}
