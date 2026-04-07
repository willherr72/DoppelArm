import { Vector3 } from 'three';
import { getEndEffectorPosition, getJointWorldPosition, getJointWorldAxis } from '$lib/utils/fk';
import { JOINT_LIMITS, rawToRadians } from '$lib/stores/joints';

export interface IKResult {
  /** Solved joint angles in radians */
  angles: number[];
  /** Whether the target was reached within tolerance */
  reached: boolean;
  /** Number of iterations used */
  iterations: number;
  /** Final distance to target in meters */
  distance: number;
}

/**
 * CCD (Cyclic Coordinate Descent) IK solver.
 *
 * Iterates from the wrist back to the shoulder, rotating each joint
 * to minimize the distance between the end-effector and the target.
 *
 * @param target - Target position in world space (meters)
 * @param currentAngles - Current joint angles in radians (6 values)
 * @param maxIterations - Maximum solver iterations
 * @param tolerance - Distance tolerance in meters
 */
export function solveCCD(
  target: Vector3,
  currentAngles: number[],
  maxIterations: number = 50,
  tolerance: number = 0.003 // 3mm
): IKResult {
  // Work with a copy
  const angles = [...currentAngles];

  // Convert joint limits from raw to radians
  const limitsRad = JOINT_LIMITS.map(([min, max]) => [
    rawToRadians(min),
    rawToRadians(max),
  ]);

  let reached = false;
  let iterations = 0;
  let distance = Infinity;

  for (let iter = 0; iter < maxIterations; iter++) {
    iterations = iter + 1;

    const ee = getEndEffectorPosition(angles);
    distance = ee.distanceTo(target);

    if (distance < tolerance) {
      reached = true;
      break;
    }

    // Iterate joints from wrist_roll (4) back to shoulder_pan (0)
    // Skip gripper (5) as it doesn't affect end-effector position
    for (let j = 4; j >= 0; j--) {
      const jointPos = getJointWorldPosition(angles, j);
      const jointAxis = getJointWorldAxis(angles, j);

      const currentEE = getEndEffectorPosition(angles);
      const toEE = currentEE.clone().sub(jointPos);
      const toTarget = target.clone().sub(jointPos);

      // Project both vectors onto the plane perpendicular to the joint axis
      const toEEProj = toEE.clone().sub(
        jointAxis.clone().multiplyScalar(toEE.dot(jointAxis))
      );
      const toTargetProj = toTarget.clone().sub(
        jointAxis.clone().multiplyScalar(toTarget.dot(jointAxis))
      );

      if (toEEProj.length() < 0.001 || toTargetProj.length() < 0.001) {
        continue;
      }

      toEEProj.normalize();
      toTargetProj.normalize();

      // Compute signed angle between the two projected vectors
      let cosAngle = toEEProj.dot(toTargetProj);
      cosAngle = Math.max(-1, Math.min(1, cosAngle));
      let angle = Math.acos(cosAngle);

      // Determine sign using cross product
      const cross = new Vector3().crossVectors(toEEProj, toTargetProj);
      if (cross.dot(jointAxis) < 0) {
        angle = -angle;
      }

      // Apply angle change with damping for stability
      const damping = 0.8;
      angles[j] += angle * damping;

      // Clamp to joint limits
      angles[j] = Math.max(limitsRad[j][0], Math.min(limitsRad[j][1], angles[j]));
    }
  }

  const finalEE = getEndEffectorPosition(angles);
  distance = finalEE.distanceTo(target);

  return {
    angles,
    reached: distance < tolerance,
    iterations,
    distance,
  };
}
