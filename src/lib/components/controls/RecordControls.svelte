<script lang="ts">
  import { isRecording, isPlaying, currentRecording, recordingName } from '$lib/stores/recording';
  import { isMirroring } from '$lib/stores/joints';
  import {
    startRecording,
    stopRecording,
    saveRecording,
    loadRecording,
    startPlayback,
    stopPlayback,
  } from '$lib/tauri/commands';
  import { showError, showStatus } from '$lib/stores/app';

  async function toggleRecording() {
    if ($isRecording) {
      try {
        const recording = await stopRecording();
        currentRecording.set(recording);
        isRecording.set(false);
        showStatus(`Recorded ${recording.frames.length} frames (${(recording.duration_ms / 1000).toFixed(1)}s)`);
      } catch (e) {
        showError(`Stop recording failed: ${e}`);
      }
    } else {
      try {
        const name = $recordingName || `recording_${Date.now()}`;
        await startRecording(name);
        isRecording.set(true);
        showStatus('Recording started');
      } catch (e) {
        showError(`Start recording failed: ${e}`);
      }
    }
  }

  async function save() {
    if (!$currentRecording) return;
    try {
      const path = `recordings/${$currentRecording.name}.doppel.json`;
      await saveRecording($currentRecording, path);
      showStatus('Recording saved');
    } catch (e) {
      showError(`Save failed: ${e}`);
    }
  }

  async function load() {
    try {
      const path = prompt('Enter recording file path:');
      if (!path) return;
      const recording = await loadRecording(path);
      currentRecording.set(recording);
      showStatus(`Loaded recording: ${recording.name} (${recording.frames.length} frames)`);
    } catch (e) {
      showError(`Load failed: ${e}`);
    }
  }

  async function togglePlayback() {
    if ($isPlaying) {
      try {
        await stopPlayback();
        isPlaying.set(false);
        showStatus('Playback stopped');
      } catch (e) {
        showError(`Stop playback failed: ${e}`);
      }
    } else if ($currentRecording) {
      try {
        await startPlayback($currentRecording);
        isPlaying.set(true);
        showStatus('Playback started');
      } catch (e) {
        showError(`Start playback failed: ${e}`);
      }
    }
  }
</script>

<div class="record-controls">
  <h3>Recording</h3>

  <div class="name-input">
    <input
      type="text"
      placeholder="Recording name..."
      bind:value={$recordingName}
      disabled={$isRecording}
    />
  </div>

  <div class="buttons">
    <button
      class="btn"
      class:recording={$isRecording}
      on:click={toggleRecording}
      disabled={!$isMirroring && !$isRecording}
      title={!$isMirroring ? 'Start mirroring first' : ''}
    >
      {$isRecording ? 'Stop Recording' : 'Record'}
    </button>

    <button class="btn" on:click={save} disabled={!$currentRecording}>
      Save
    </button>

    <button class="btn" on:click={load}>
      Load
    </button>

    <button
      class="btn"
      class:playing={$isPlaying}
      on:click={togglePlayback}
      disabled={!$currentRecording && !$isPlaying}
    >
      {$isPlaying ? 'Stop' : 'Play'}
    </button>
  </div>

  {#if $currentRecording}
    <div class="recording-info">
      <span>{$currentRecording.name}</span>
      <span>{$currentRecording.frames.length} frames</span>
      <span>{($currentRecording.duration_ms / 1000).toFixed(1)}s</span>
    </div>
  {/if}
</div>

<style>
  .record-controls {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  h3 {
    margin: 0;
    color: #e0e0e0;
    font-size: 14px;
  }

  .name-input input {
    width: 100%;
    padding: 6px 10px;
    background: #1a1a2e;
    border: 1px solid #444;
    border-radius: 4px;
    color: #e0e0e0;
    font-size: 12px;
  }

  .buttons {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }

  .btn {
    padding: 8px;
    border: 1px solid #444;
    border-radius: 6px;
    background: #1a1a2e;
    color: #ccc;
    font-size: 12px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .btn:hover:not(:disabled) {
    background: #16213e;
    border-color: #4a90d9;
  }

  .btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .btn.recording {
    background: #dc2626;
    border-color: #dc2626;
    color: white;
    animation: pulse 1s infinite;
  }

  .btn.playing {
    background: #16a34a;
    border-color: #16a34a;
    color: white;
  }

  .recording-info {
    display: flex;
    gap: 12px;
    font-size: 11px;
    color: #888;
    padding: 4px 0;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.7; }
  }
</style>
