import { Matrix4, Vector3 } from 'three';
import { LINKS } from './arm-config';

/**
 * Compute the transform matrix for each joint given joint angles in radians.
 * Returns an array of 6 world-space positions (Vector3) for each joint center,
 * plus the end-effector position (7 total).
 *
 * The arm is modeled as:
 *   Base (origin) -> shoulder_pan (Y-rot) -> up by base height ->
 *   shoulder_lift (Z-rot) -> forward by upperArm ->
 *   elbow_flex (Z-rot) -> forward by forearm ->
 *   wrist_flex (Z-rot) ->
 *   wrist_roll (X-rot) -> forward by wristToEE ->
 *   end-effector
 */
export function computeJointPositions(angles: number[]): Vector3[] {
  const positions: Vector3[] = [];
  const mat = new Matrix4();
  mat.identity();

  // Adjust angles relative to center position
  const a = angles;

  // Base position
  positions.push(new Vector3(0, 0, 0));

  // Joint 0: shoulder pan - rotate around Y, then translate up
  mat.multiply(new Matrix4().makeRotationY(a[0]));
  mat.multiply(new Matrix4().makeTranslation(0, LINKS.base, 0));
  positions.push(new Vector3().setFromMatrixPosition(mat));

  // Joint 1: shoulder lift - rotate around Z, then translate along X (upper arm)
  mat.multiply(new Matrix4().makeRotationZ(a[1]));
  mat.multiply(new Matrix4().makeTranslation(LINKS.upperArm, 0, 0));
  positions.push(new Vector3().setFromMatrixPosition(mat));

  // Joint 2: elbow flex - rotate around Z, then translate along X (forearm)
  mat.multiply(new Matrix4().makeRotationZ(a[2]));
  mat.multiply(new Matrix4().makeTranslation(LINKS.forearm, 0, 0));
  positions.push(new Vector3().setFromMatrixPosition(mat));

  // Joint 3: wrist flex - rotate around Z
  mat.multiply(new Matrix4().makeRotationZ(a[3]));
  positions.push(new Vector3().setFromMatrixPosition(mat));

  // Joint 4: wrist roll - rotate around X, then translate along X (wrist to EE)
  mat.multiply(new Matrix4().makeRotationX(a[4]));
  mat.multiply(new Matrix4().makeTranslation(LINKS.wristToEE, 0, 0));
  positions.push(new Vector3().setFromMatrixPosition(mat));

  // End-effector position
  positions.push(new Vector3().setFromMatrixPosition(mat));

  return positions;
}

/**
 * Get the end-effector world position for a set of joint angles.
 */
export function getEndEffectorPosition(angles: number[]): Vector3 {
  const positions = computeJointPositions(angles);
  return positions[positions.length - 1];
}

/**
 * Compute the world-space rotation axis for a given joint index,
 * given the current set of joint angles.
 */
export function getJointWorldAxis(angles: number[], jointIndex: number): Vector3 {
  const a = angles;
  const mat = new Matrix4();
  mat.identity();

  // Build transform chain up to (but not including) the target joint
  if (jointIndex > 0) {
    mat.multiply(new Matrix4().makeRotationY(a[0]));
    mat.multiply(new Matrix4().makeTranslation(0, LINKS.base, 0));
  }
  if (jointIndex > 1) {
    mat.multiply(new Matrix4().makeRotationZ(a[1]));
    mat.multiply(new Matrix4().makeTranslation(LINKS.upperArm, 0, 0));
  }
  if (jointIndex > 2) {
    mat.multiply(new Matrix4().makeRotationZ(a[2]));
    mat.multiply(new Matrix4().makeTranslation(LINKS.forearm, 0, 0));
  }
  if (jointIndex > 3) {
    mat.multiply(new Matrix4().makeRotationZ(a[3]));
  }
  if (jointIndex > 4) {
    mat.multiply(new Matrix4().makeRotationX(a[4]));
    mat.multiply(new Matrix4().makeTranslation(LINKS.wristToEE, 0, 0));
  }

  // The local rotation axis for each joint:
  const localAxes: Vector3[] = [
    new Vector3(0, 1, 0), // shoulder_pan: Y
    new Vector3(0, 0, 1), // shoulder_lift: Z
    new Vector3(0, 0, 1), // elbow_flex: Z
    new Vector3(0, 0, 1), // wrist_flex: Z
    new Vector3(1, 0, 0), // wrist_roll: X
    new Vector3(0, 0, 1), // gripper: Z
  ];

  const axis = localAxes[jointIndex].clone();
  axis.transformDirection(mat);
  return axis;
}

/**
 * Get the world-space position of a specific joint.
 */
export function getJointWorldPosition(angles: number[], jointIndex: number): Vector3 {
  const positions = computeJointPositions(angles);
  return positions[Math.min(jointIndex + 1, positions.length - 1)];
}
