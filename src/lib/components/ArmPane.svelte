<script lang="ts">
  import { onDestroy } from 'svelte';
  import SingleArmScene from './scene/SingleArmScene.svelte';
  import JointSliders from './controls/JointSliders.svelte';
  import { availablePorts, leaderConnection, followerConnection } from '$lib/stores/connection';
  import type { ArmConnection } from '$lib/stores/connection';
  import { leaderAngles, followerAngles, leaderJoints, followerJoints, isMirroring } from '$lib/stores/joints';
  import { listPorts, connectArm, disconnectArm, scanMotors, readAllJoints, diagnosePort } from '$lib/tauri/commands';
  import { showError, showStatus } from '$lib/stores/app';
  import type { Writable } from 'svelte/store';

  export let role: 'leader' | 'follower';
  export let color: string;

  $: connection = role === 'leader' ? $leaderConnection : $followerConnection;
  $: angles = role === 'leader' ? $leaderAngles : $followerAngles;
  $: connectionStore = (role === 'leader' ? leaderConnection : followerConnection) as Writable<ArmConnection | null>;

  let selectedPort = '';
  let connecting = false;
  let pollInterval: ReturnType<typeof setInterval> | null = null;

  // Only poll when motors are confirmed present (motorIds populated)
  $: {
    if (connection?.connected && connection.motorIds.length > 0 && !$isMirroring) {
      startPolling();
    } else {
      stopPolling();
    }
  }

  let pollBusy = false;

  function startPolling() {
    if (pollInterval) return;
    pollInterval = setInterval(async () => {
      if ($isMirroring || pollBusy) return;
      pollBusy = true;
      try {
        const pos = await readAllJoints(role);
        const store = role === 'leader' ? leaderJoints : followerJoints;
        store.update(s => ({ ...s, positions: pos, timestamp: Date.now() }));
      } catch {
        // Silently ignore poll errors
      } finally {
        pollBusy = false;
      }
    }, 250); // 4Hz polling, skips if previous poll still running
  }

  function stopPolling() {
    if (pollInterval) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
  }

  onDestroy(() => stopPolling());

  async function connect() {
    if (!selectedPort) return;
    connecting = true;
    try {
      await connectArm(selectedPort, role);
      connectionStore.set({ port: selectedPort, baudRate: 1000000, connected: true, motorIds: [] });
      showStatus(`${role} arm connected`);
      try {
        const ids = await scanMotors(selectedPort);
        connectionStore.update(c => c ? { ...c, motorIds: ids } : c);
      } catch { /* motors may not be configured yet */ }
    } catch (e) {
      showError(`Connection failed: ${e}`);
    } finally {
      connecting = false;
    }
  }

  async function disconnect() {
    stopPolling();
    try {
      await disconnectArm(role);
      connectionStore.set(null);
      showStatus(`${role} arm disconnected`);
    } catch (e) {
      showError(`Disconnect failed: ${e}`);
    }
  }

  let diagnosing = false;
  let diagResults: string[] = [];

  async function diagnose() {
    const port = connection?.port || selectedPort;
    if (!port) return;
    diagnosing = true;
    diagResults = [];
    try {
      diagResults = await diagnosePort(port);
    } catch (e) {
      showError(`Diagnose failed: ${e}`);
    } finally {
      diagnosing = false;
    }
  }
</script>

<div class="arm-pane">
  <div class="pane-header">
    <h2 style="color: {color}">{role === 'leader' ? 'Leader' : 'Follower'}</h2>
    <div class="connection-info">
      {#if connection?.connected}
        <span class="status connected">{connection.port}</span>
        <span class="motor-count">{connection.motorIds.length} motors</span>
        <button class="btn-sm" on:click={disconnect}>Disconnect</button>
      {:else}
        <select bind:value={selectedPort}>
          <option value="">Select port</option>
          {#each $availablePorts as port}
            <option value={port.name}>
              {port.name} {port.description ? `(${port.description})` : ''}
            </option>
          {/each}
        </select>
        <button class="btn-sm primary" on:click={connect} disabled={!selectedPort || connecting}>
          {connecting ? 'Connecting...' : 'Connect'}
        </button>
        <button class="btn-sm" on:click={diagnose} disabled={!selectedPort || diagnosing}>
          {diagnosing ? '...' : 'Diagnose'}
        </button>
      {/if}
    </div>
  </div>

  {#if diagResults.length > 0}
    <div class="diag-results">
      {#each diagResults as line}
        <div class="diag-line" class:ok={line.includes('OK')}>{line}</div>
      {/each}
    </div>
  {/if}

  <div class="scene-wrapper">
    <SingleArmScene {angles} {color} />
  </div>

  <div class="controls-area">
    <JointSliders {role} readonly={role === 'leader'} />
  </div>
</div>

<style>
  .arm-pane {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .pane-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 16px;
    border-bottom: 1px solid #1e2a3a;
    flex-shrink: 0;
  }

  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .connection-info {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }

  .status {
    padding: 2px 8px;
    border-radius: 3px;
    font-family: monospace;
    font-size: 11px;
  }

  .status.connected {
    background: #0d3320;
    color: #4ade80;
  }

  .motor-count {
    color: #666;
    font-size: 11px;
  }

  .diag-results {
    padding: 6px 12px;
    background: #0a0e14;
    border-bottom: 1px solid #1e2a3a;
    font-family: monospace;
    font-size: 10px;
  }

  .diag-line {
    color: #667;
    padding: 1px 0;
  }

  .diag-line.ok {
    color: #4ade80;
  }

  select {
    padding: 4px 8px;
    background: #111827;
    color: #e0e0e0;
    border: 1px solid #2a3444;
    border-radius: 4px;
    font-size: 11px;
  }

  .btn-sm {
    padding: 4px 10px;
    border: 1px solid #2a3444;
    border-radius: 4px;
    background: #111827;
    color: #aaa;
    font-size: 11px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .btn-sm:hover:not(:disabled) {
    background: #1a2332;
    color: #fff;
  }

  .btn-sm.primary {
    border-color: #2563eb;
    color: #60a5fa;
  }

  .btn-sm.primary:hover:not(:disabled) {
    background: #1e3a5f;
  }

  .btn-sm:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .scene-wrapper {
    flex: 1;
    min-height: 0;
    padding: 8px;
  }

  .controls-area {
    flex-shrink: 0;
    max-height: 280px;
    overflow-y: auto;
    padding: 12px 16px;
    border-top: 1px solid #1e2a3a;
  }
</style>
