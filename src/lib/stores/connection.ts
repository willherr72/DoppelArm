import { writable, derived } from 'svelte/store';

export interface PortInfo {
  name: string;
  description: string;
  vid: number | null;
  pid: number | null;
}

export interface ArmConnection {
  port: string;
  baudRate: number;
  connected: boolean;
  motorIds: number[];
}

export const availablePorts = writable<PortInfo[]>([]);
export const leaderConnection = writable<ArmConnection | null>(null);
export const followerConnection = writable<ArmConnection | null>(null);

export const bothConnected = derived(
  [leaderConnection, followerConnection],
  ([$leader, $follower]) => $leader?.connected === true && $follower?.connected === true
);
