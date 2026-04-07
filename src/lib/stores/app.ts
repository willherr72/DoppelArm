import { writable } from 'svelte/store';

export type AppMode = 'setup' | 'calibration' | 'control' | 'record' | 'ik';

export const currentMode = writable<AppMode>('setup');
export const error = writable<string | null>(null);
export const statusMessage = writable<string>('');

export function showError(msg: string) {
  error.set(msg);
  setTimeout(() => error.set(null), 5000);
}

export function showStatus(msg: string) {
  statusMessage.set(msg);
  setTimeout(() => statusMessage.set(''), 3000);
}
