/** SO-ARM100 kinematic configuration */

/** Link lengths in meters */
export const LINKS = {
  base: 0.055,       // Base height to shoulder
  upperArm: 0.104,   // Shoulder to elbow
  forearm: 0.095,     // Elbow to wrist
  wristToEE: 0.070,  // Wrist to end-effector
} as const;

/**
 * Joint rotation axes and directions.
 * Each joint rotates around a specific axis in the parent frame.
 *
 * Joint 0 (shoulder_pan): Rotates around Y (vertical)
 * Joint 1 (shoulder_lift): Rotates around Z (horizontal, perpendicular to arm plane)
 * Joint 2 (elbow_flex): Rotates around Z
 * Joint 3 (wrist_flex): Rotates around Z
 * Joint 4 (wrist_roll): Rotates around X (along forearm axis)
 * Joint 5 (gripper): Opens/closes (no FK contribution beyond visualization)
 */
export const JOINT_AXES = [
  { axis: 'y' as const, offset: [0, 0, 0] as [number, number, number] },
  { axis: 'z' as const, offset: [0, 0, 0] as [number, number, number] },
  { axis: 'z' as const, offset: [0, 0, 0] as [number, number, number] },
  { axis: 'z' as const, offset: [0, 0, 0] as [number, number, number] },
  { axis: 'x' as const, offset: [0, 0, 0] as [number, number, number] },
  { axis: 'z' as const, offset: [0, 0, 0] as [number, number, number] },
] as const;

/** Colors for the two arms */
export const ARM_COLORS = {
  leader: '#4a90d9',     // Steel blue
  follower: '#e8734a',   // Coral/orange
} as const;

/** Joint angle offsets to align the "center" position (2048) with zero-angle */
export const CENTER_OFFSET = Math.PI; // 2048/4095 * 2PI ≈ PI
