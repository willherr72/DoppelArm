<script lang="ts">
  import { onDestroy } from 'svelte';
  import SingleArmScene from './scene/SingleArmScene.svelte';
  import JointSliders from './controls/JointSliders.svelte';
  import SetupModal from './setup/SetupModal.svelte';
  import { availablePorts, leaderConnection, followerConnection } from '$lib/stores/connection';
  import type { ArmConnection } from '$lib/stores/connection';
  import { leaderAngles, followerAngles, leaderJoints, followerJoints, isMirroring } from '$lib/stores/joints';
  import { listPorts, connectArm, disconnectArm, scanMotors, scanConnected, readAllJoints, diagnosePort, resetPositionCorrections } from '$lib/tauri/commands';
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
      // Scan via the connected bus (port is already open, can't reopen)
      try {
        const ids = await scanConnected(role);
        connectionStore.update(c => c ? { ...c, motorIds: ids } : c);
      } catch (e) {
        console.warn('Post-connect scan failed:', e);
      }
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
  let scanning = false;
  let resultLines: string[] = [];
  let resultTitle = '';

  async function diagnose() {
    const port = connection?.port || selectedPort;
    if (!port) return;
    diagnosing = true;
    resultLines = [];
    resultTitle = 'Diagnose';
    try {
      resultLines = await diagnosePort(port);
    } catch (e) {
      showError(`Diagnose failed: ${e}`);
    } finally {
      diagnosing = false;
    }
  }

  let setupOpen = false;
  let resettingOffsets = false;

  function openSetup() {
    if (!selectedPort) return;
    setupOpen = true;
  }

  async function resetOffsets() {
    if (!connection?.connected) return;
    if (!confirm(`Reset all position offsets on ${role}?\n\nThis clears any prior recenter and disables torque first so the servos won't lurch.`)) {
      return;
    }
    resettingOffsets = true;
    try {
      await resetPositionCorrections(role);
      showStatus(`${role} position offsets reset to 0`);
    } catch (e) {
      showError(`Reset failed: ${e}`);
    } finally {
      resettingOffsets = false;
    }
  }

  async function scanBus() {
    scanning = true;
    resultLines = [];
    resultTitle = 'Scan IDs';
    try {
      // Use connected bus if already connected, else open fresh port
      const ids = connection?.connected
        ? await scanConnected(role)
        : await scanMotors(selectedPort);
      if (ids.length === 0) {
        resultLines = ['No motors found on the bus'];
      } else {
        resultLines = [
          `Found ${ids.length} motor${ids.length === 1 ? '' : 's'}: IDs ${ids.join(', ')}`,
        ];
        const expected = [1, 2, 3, 4, 5, 6];
        const missing = expected.filter((id) => !ids.includes(id));
        const extra = ids.filter((id) => !expected.includes(id));
        if (missing.length > 0) {
          resultLines.push(`Missing IDs (need 1-6): ${missing.join(', ')}`);
        }
        if (extra.length > 0) {
          resultLines.push(`Extra IDs (outside 1-6): ${extra.join(', ')}`);
        }
        if (missing.length === 0 && extra.length === 0) {
          resultLines.push('All 6 motors configured correctly');
        }
      }
      // Update connection store with found IDs so polling starts
      if (connection?.connected) {
        connectionStore.update((c) => (c ? { ...c, motorIds: ids } : c));
      }
    } catch (e) {
      showError(`Scan failed: ${e}`);
    } finally {
      scanning = false;
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
        <button class="btn-sm" on:click={scanBus} disabled={scanning}>
          {scanning ? '...' : 'Rescan'}
        </button>
        <button class="btn-sm" on:click={resetOffsets} disabled={resettingOffsets}>
          {resettingOffsets ? '...' : 'Reset offsets'}
        </button>
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
        <button class="btn-sm" on:click={scanBus} disabled={!selectedPort || scanning}>
          {scanning ? '...' : 'Scan IDs'}
        </button>
        <button class="btn-sm" on:click={openSetup} disabled={!selectedPort}>
          Setup IDs
        </button>
      {/if}
    </div>
  </div>

  <SetupModal
    bind:open={setupOpen}
    port={selectedPort}
    armLabel={role === 'leader' ? 'Leader' : 'Follower'}
  />

  {#if resultLines.length > 0}
    <div class="diag-results">
      <div class="diag-title">
        {resultTitle}
        <button class="dismiss-btn" on:click={() => (resultLines = [])} aria-label="Dismiss">×</button>
      </div>
      {#each resultLines as line}
        <div
          class="diag-line"
          class:ok={line.includes('OK') || line.includes('Found') || line.includes('correctly')}
          class:warn={line.includes('Missing') || line.includes('Extra') || line.includes('No motors')}
        >
          {line}
        </div>
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

  .diag-title {
    display: flex;
    justify-content: space-between;
    align-items: center;
    color: #8899aa;
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    font-weight: 700;
    margin-bottom: 4px;
    padding-bottom: 3px;
    border-bottom: 1px solid #1e2a3a;
  }

  .dismiss-btn {
    background: none;
    border: none;
    color: #556677;
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
    padding: 0 4px;
  }

  .dismiss-btn:hover {
    color: #fff;
  }

  .diag-line {
    color: #667;
    padding: 1px 0;
  }

  .diag-line.ok {
    color: #4ade80;
  }

  .diag-line.warn {
    color: #facc15;
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
