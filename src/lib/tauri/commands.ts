import { invoke, Channel } from '@tauri-apps/api/core';
import type { PortInfo } from '$lib/stores/connection';
import type { Recording } from '$lib/stores/recording';

// ---- Serial / Connection ----

export async function listPorts(): Promise<PortInfo[]> {
  return invoke('list_ports');
}

export async function scanMotors(port: string, baudRate?: number): Promise<number[]> {
  return invoke('scan_motors', { port, baudRate });
}

export async function scanConnected(role: 'leader' | 'follower'): Promise<number[]> {
  return invoke('scan_connected', { role });
}

export async function connectArm(port: string, role: 'leader' | 'follower', baudRate?: number): Promise<void> {
  return invoke('connect_arm', { port, role, baudRate });
}

export async function disconnectArm(role: 'leader' | 'follower'): Promise<void> {
  return invoke('disconnect_arm', { role });
}

export async function configureMotor(port: string, currentId: number, newId: number, baudRate?: number): Promise<void> {
  return invoke('configure_motor', { port, currentId, newId, baudRate });
}

export async function autoDetectMotor(port: string): Promise<[number, number]> {
  return invoke('auto_detect_motor', { port });
}

export async function diagnosePort(port: string): Promise<string[]> {
  return invoke('diagnose_port', { port });
}

// ---- Arm Control ----

export async function readAllJoints(role: 'leader' | 'follower'): Promise<number[]> {
  return invoke('read_all_joints', { role });
}

export async function writeAllJoints(role: 'leader' | 'follower', positions: number[]): Promise<void> {
  return invoke('write_all_joints', { role, positions });
}

export async function writeSingleJoint(role: 'leader' | 'follower', jointIndex: number, position: number): Promise<void> {
  return invoke('write_single_joint', { role, jointIndex, position });
}

export async function setTorque(role: 'leader' | 'follower', enabled: boolean): Promise<void> {
  return invoke('set_torque', { role, enabled });
}

export interface ServoStatus {
  id: number;
  position: number;
  speed: number;
  load: number;
  voltage: number;
  temperature: number;
}

export async function readServoStatus(role: 'leader' | 'follower', servoId: number): Promise<ServoStatus> {
  return invoke('read_servo_status', { role, servoId });
}

export async function enableContinuousRotation(role: 'leader' | 'follower', servoId: number): Promise<void> {
  return invoke('enable_continuous_rotation', { role, servoId });
}

export async function enableSingleTurn(role: 'leader' | 'follower', servoId: number): Promise<void> {
  return invoke('enable_single_turn', { role, servoId });
}

export async function setJointLimit(
  role: 'leader' | 'follower',
  jointIndex: number,
  min: number,
  max: number
): Promise<void> {
  return invoke('set_joint_limit', { role, jointIndex, min, max });
}

export async function getJointLimits(role: 'leader' | 'follower'): Promise<[number, number][]> {
  return invoke('get_joint_limits', { role });
}

export async function resetPositionCorrections(role: 'leader' | 'follower'): Promise<void> {
  return invoke('reset_position_corrections', { role });
}

// ---- Calibration ----

export async function calibrateCapture(role: 'leader' | 'follower'): Promise<number[]> {
  return invoke('calibrate_capture', { role });
}

export async function computeCalibration(): Promise<number[]> {
  return invoke('compute_calibration');
}

export async function saveCalibration(): Promise<void> {
  return invoke('save_calibration');
}

export async function loadCalibration(): Promise<number[]> {
  return invoke('load_calibration');
}

export async function getCalibration(): Promise<number[]> {
  return invoke('get_calibration');
}

// ---- Mirroring ----

export interface JointUpdatePayload {
  leader: number[];
  follower: number[];
  timestamp_ms: number;
}

export async function startMirroring(
  onUpdate: (payload: JointUpdatePayload) => void
): Promise<void> {
  const channel = new Channel<JointUpdatePayload>();
  channel.onmessage = onUpdate;
  return invoke('start_mirroring', { onUpdate: channel });
}

export async function stopMirroring(): Promise<void> {
  return invoke('stop_mirroring');
}

// ---- Recording ----

export async function startRecording(name: string): Promise<void> {
  return invoke('start_recording', { name });
}

export async function stopRecording(): Promise<Recording> {
  return invoke('stop_recording');
}

export async function saveRecording(recording: Recording, path: string): Promise<void> {
  return invoke('save_recording', { recording, path });
}

export async function loadRecording(path: string): Promise<Recording> {
  return invoke('load_recording', { path });
}

export async function startPlayback(recording: Recording): Promise<void> {
  return invoke('start_playback', { recording });
}

export async function stopPlayback(): Promise<void> {
  return invoke('stop_playback');
}
