import { writable } from 'svelte/store';
import type { CalibrationState } from '$lib/tauri/commands';
import { getCalibrationState } from '$lib/tauri/commands';

const NUM_JOINTS = 6;
const CENTER = 2048;

function defaultCalibration(): CalibrationState {
  return {
    offsets: Array(NUM_JOINTS).fill(0),
    mirror_signs: Array(NUM_JOINTS).fill(1),
    leader_reference: Array(NUM_JOINTS).fill(CENTER),
    follower_reference: Array(NUM_JOINTS).fill(CENTER),
    leader_port: '',
    follower_port: '',
  };
}

export const calibrationState = writable<CalibrationState>(defaultCalibration());

/** Pull the latest calibration state from the Rust backend. */
export async function refreshCalibrationState(): Promise<void> {
  try {
    const state = await getCalibrationState();
    calibrationState.set(state);
  } catch (e) {
    console.warn('Failed to refresh calibration state:', e);
  }
}
