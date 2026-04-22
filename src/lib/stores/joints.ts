import { writable, derived } from 'svelte/store';
import { calibrationState } from './calibration';
import { computeVisualAngles } from '$lib/utils/visual-config';

const NUM_JOINTS = 6;
const CENTER = 2048;

export interface JointState {
  positions: number[];
  timestamp: number;
}

function defaultJointState(): JointState {
  return {
    positions: Array(NUM_JOINTS).fill(CENTER),
    timestamp: 0,
  };
}

export const leaderJoints = writable<JointState>(defaultJointState());
export const followerJoints = writable<JointState>(defaultJointState());
export const isMirroring = writable<boolean>(false);

export const JOINT_NAMES = [
  'Shoulder Pan',
  'Shoulder Lift',
  'Elbow Flex',
  'Wrist Flex',
  'Wrist Roll',
  'Gripper',
];

export const JOINT_LIMITS: [number, number][] = [
  [0, 4095],
  [512, 3584],
  [512, 3584],
  [512, 3584],
  [0, 4095],
  [1400, 2700],
];

/** Convert raw position (0-4095) to radians (0 to 2*PI) */
export function rawToRadians(raw: number): number {
  return (raw / 4095) * 2 * Math.PI;
}

/** Convert raw position to degrees (0-360) */
export function rawToDegrees(raw: number): number {
  return (raw / 4095) * 360;
}

/**
 * 3D-ready joint angles. Computed against the per-arm calibration
 * reference so that "at rest" the model sits in the configured home
 * pose and motion away from rest tracks the real arm one-to-one.
 */
export const leaderAngles = derived(
  [leaderJoints, calibrationState],
  ([$lj, $cal]) => computeVisualAngles($lj.positions, $cal.leader_reference),
);

export const followerAngles = derived(
  [followerJoints, calibrationState],
  ([$fj, $cal]) => computeVisualAngles($fj.positions, $cal.follower_reference),
);
