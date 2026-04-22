/**
 * Per-joint configuration for converting raw servo encoder values
 * (0-4095) into 3D model joint rotations (radians).
 *
 *   visual_angle[i] = VISUAL_HOME_ROTATION[i]
 *                   + VISUAL_SIGN[i] * (raw[i] - home_raw[i]) / 4095 * 2π
 *
 * - home_raw[i] comes from the per-arm calibration reference pose
 *   (CalibrationState.leader_reference / .follower_reference). When the
 *   real arm is at its calibrated reference, visual_angle equals
 *   VISUAL_HOME_ROTATION exactly — so the 3D model's neutral pose is
 *   what we draw at the calibrated rest.
 * - VISUAL_SIGN flips per-joint encoder direction when the servo counts
 *   opposite to the visual rotation we want.
 * - VISUAL_HOME_ROTATION places each joint into the SO-ARM100's typical
 *   resting (folded) configuration so the 3D mirrors what you see on
 *   the table when no commands are being sent.
 *
 * Tuning workflow: put the physical arm at rest, run calibrate-capture
 * so the reference matches the rest pose, then nudge VISUAL_HOME_ROTATION
 * here until the on-screen pose matches the photo. Sign flips show up as
 * "the joint moves the wrong way when I move the real one."
 */

const D = (deg: number) => (deg * Math.PI) / 180;

/** Radians added to each joint at the calibrated rest pose.
 *  URDF zero-pose is the visual neutral now, so all entries default to 0;
 *  tweak per-joint if the calibrated reference shouldn't be the URDF zero. */
export const VISUAL_HOME_ROTATION: readonly number[] = [Math.PI, 0, 0, 0, 0, 0] as const;

/** Direction multiplier per joint. +1 = encoder and visual agree, -1 = flip.
 *  Reset for the URDF model — joint axes follow URDF conventions, may need
 *  re-flipping based on observed motion. */
export const VISUAL_SIGN: readonly number[] = [1, -1, 1, 1, -1, 1] as const;

/**
 * Per-joint gain — encoder counts to joint radians.
 * 1.0 means 4096 raw counts = 360° of joint motion (direct-drive servo).
 * Use >1 if the joint moves more than the encoder suggests (gear reduction
 * between servo and joint), <1 if the joint moves less.
 */
export const VISUAL_GAIN: readonly number[] = [1, 1, 1, 1, 1, 1] as const;

/**
 * Gripper isn't a rotation joint — it's an open/close mechanism. Its
 * "angle" output is a normalized open amount in [0, 1] derived from the
 * absolute raw position, so the model maps to the physical mechanism
 * regardless of where the calibration reference happened to land.
 */
const GRIPPER_INDEX = 5;
const GRIPPER_RAW_CLOSED = 1400;
const GRIPPER_RAW_OPEN = 2700;

/**
 * Compute 3D joint angles (radians) from raw servo positions, using
 * the per-arm calibration reference as the home pose.
 */
export function computeVisualAngles(
  rawPositions: number[],
  homeReference: number[],
): number[] {
  const out = new Array(rawPositions.length);
  for (let i = 0; i < rawPositions.length; i++) {
    const raw = rawPositions[i] ?? 2048;

    if (i === GRIPPER_INDEX) {
      const span = GRIPPER_RAW_OPEN - GRIPPER_RAW_CLOSED;
      const t = (raw - GRIPPER_RAW_CLOSED) / span;
      out[i] = Math.min(1, Math.max(0, t));
      continue;
    }

    const home = homeReference[i] ?? 2048;
    const gain = VISUAL_GAIN[i] ?? 1;
    const delta = ((raw - home) / 4095) * 2 * Math.PI * gain;
    out[i] = (VISUAL_HOME_ROTATION[i] ?? 0) + (VISUAL_SIGN[i] ?? 1) * delta;
  }
  return out;
}
