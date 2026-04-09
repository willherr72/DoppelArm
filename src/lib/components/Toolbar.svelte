<script lang="ts">
  import { isMirroring } from '$lib/stores/joints';
  import { isRecording, isPlaying, currentRecording, recordingName } from '$lib/stores/recording';
  import { leaderConnection, followerConnection, bothConnected, availablePorts } from '$lib/stores/connection';
  import { error, statusMessage } from '$lib/stores/app';
  import {
    listPorts,
    startMirroring, stopMirroring,
    startRecording, stopRecording, saveRecording, loadRecording,
    startPlayback, stopPlayback,
  } from '$lib/tauri/commands';
  import { leaderJoints, followerJoints } from '$lib/stores/joints';
  import { showError, showStatus } from '$lib/stores/app';
  import CalibrationModal from './calibration/CalibrationModal.svelte';

  let calibrationOpen = false;
  let calibrationDone = false;

  async function refreshPorts() {
    try {
      const ports = await listPorts();
      availablePorts.set(ports);
      showStatus(`Found ${ports.length} ports`);
    } catch (e) {
      showError(`Port scan failed: ${e}`);
    }
  }

  async function toggleMirror() {
    if ($isMirroring) {
      try {
        await stopMirroring();
        isMirroring.set(false);
        showStatus('Mirroring stopped');
      } catch (e) { showError(`${e}`); }
    } else {
      try {
        await startMirroring((payload) => {
          leaderJoints.set({ positions: payload.leader, timestamp: payload.timestamp_ms });
          followerJoints.set({ positions: payload.follower, timestamp: payload.timestamp_ms });
        });
        isMirroring.set(true);
        showStatus('Mirroring active');
      } catch (e) { showError(`${e}`); }
    }
  }

  async function toggleRecord() {
    if ($isRecording) {
      try {
        const rec = await stopRecording();
        currentRecording.set(rec);
        isRecording.set(false);
        showStatus(`Recorded ${rec.frames.length} frames`);
      } catch (e) { showError(`${e}`); }
    } else {
      try {
        await startRecording($recordingName || `rec_${Date.now()}`);
        isRecording.set(true);
      } catch (e) { showError(`${e}`); }
    }
  }

  async function togglePlayback() {
    if ($isPlaying) {
      try { await stopPlayback(); isPlaying.set(false); } catch (e) { showError(`${e}`); }
    } else if ($currentRecording) {
      try { await startPlayback($currentRecording); isPlaying.set(true); } catch (e) { showError(`${e}`); }
    }
  }

  function openCalibration() {
    calibrationOpen = true;
  }

  refreshPorts();
</script>

<CalibrationModal bind:open={calibrationOpen} on:saved={() => (calibrationDone = true)} />

<header class="toolbar">
  <div class="left">
    <span class="app-title">DoppelArm</span>
    <button class="tool-btn" on:click={refreshPorts}>Scan Ports</button>
  </div>

  <div class="center">
    <div class="btn-group">
      <button
        class="tool-btn"
        class:active-cal={calibrationDone}
        on:click={openCalibration}
        disabled={!$bothConnected}
      >
        {calibrationDone ? 'Recalibrate' : 'Calibrate'}
      </button>
    </div>

    <div class="separator"></div>

    <button
      class="tool-btn mirror"
      class:active={$isMirroring}
      on:click={toggleMirror}
      disabled={!$bothConnected}
    >
      {$isMirroring ? 'Stop Mirror' : 'Mirror'}
    </button>

    <div class="separator"></div>

    <div class="btn-group">
      <button
        class="tool-btn record"
        class:active={$isRecording}
        on:click={toggleRecord}
        disabled={!$isMirroring && !$isRecording}
      >
        {$isRecording ? 'Stop Rec' : 'Record'}
      </button>
      <button
        class="tool-btn"
        class:active={$isPlaying}
        on:click={togglePlayback}
        disabled={!$currentRecording && !$isPlaying}
      >
        {$isPlaying ? 'Stop' : 'Play'}
      </button>
    </div>
  </div>

  <div class="right">
    {#if $error}
      <span class="msg error">{$error}</span>
    {:else if $statusMessage}
      <span class="msg info">{$statusMessage}</span>
    {/if}

    <div class="indicators">
      {#if $leaderConnection?.connected}
        <span class="ind on">L</span>
      {:else}
        <span class="ind off">L</span>
      {/if}
      {#if $followerConnection?.connected}
        <span class="ind on">F</span>
      {:else}
        <span class="ind off">F</span>
      {/if}
    </div>
  </div>
</header>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 42px;
    padding: 0 12px;
    background: #0f1520;
    border-bottom: 1px solid #1e2a3a;
    flex-shrink: 0;
    gap: 12px;
  }

  .left, .center, .right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .center {
    gap: 6px;
  }

  .right {
    gap: 12px;
    min-width: 0;
  }

  .app-title {
    font-size: 14px;
    font-weight: 700;
    color: #c0c8d8;
    letter-spacing: 0.3px;
    margin-right: 8px;
  }

  .btn-group {
    display: flex;
    gap: 2px;
  }

  .separator {
    width: 1px;
    height: 20px;
    background: #1e2a3a;
    margin: 0 4px;
  }

  .tool-btn {
    padding: 5px 12px;
    border: 1px solid #1e2a3a;
    border-radius: 4px;
    background: #111827;
    color: #8899aa;
    font-size: 12px;
    cursor: pointer;
    transition: all 0.12s;
    white-space: nowrap;
  }

  .tool-btn:hover:not(:disabled) {
    background: #1a2332;
    color: #c0d0e0;
    border-color: #2a3a4a;
  }

  .tool-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .tool-btn.mirror.active {
    background: #1e3a5f;
    border-color: #2563eb;
    color: #60a5fa;
  }

  .tool-btn.record.active {
    background: #3f1219;
    border-color: #dc2626;
    color: #f87171;
  }

  .tool-btn.active-cal {
    color: #4ade80;
    border-color: #166534;
  }

  .msg {
    font-size: 10px;
    font-family: monospace;
    max-width: 700px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
  }

  .msg.error { color: #f87171; }
  .msg.info { color: #64748b; }

  .indicators {
    display: flex;
    gap: 4px;
  }

  .ind {
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
    font-size: 10px;
    font-weight: 700;
    font-family: monospace;
  }

  .ind.on {
    background: #0d3320;
    color: #4ade80;
  }

  .ind.off {
    background: #1a1a2e;
    color: #444;
  }
</style>
