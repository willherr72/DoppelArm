import { writable } from 'svelte/store';

export interface RecordingFrame {
  t: number;
  leader: number[];
  follower: number[];
}

export interface Recording {
  version: number;
  name: string;
  created_at: string;
  duration_ms: number;
  sample_rate_hz: number;
  calibration_offsets: number[];
  frames: RecordingFrame[];
}

export const isRecording = writable<boolean>(false);
export const isPlaying = writable<boolean>(false);
export const currentRecording = writable<Recording | null>(null);
export const recordingName = writable<string>('');
