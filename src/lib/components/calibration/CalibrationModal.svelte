<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import {
    calibrateCapture,
    computeCalibration,
    saveCalibration,
    loadCalibration,
  } from '$lib/tauri/commands';
  import { showError, showStatus } from '$lib/stores/app';

  export let open: boolean = false;

  const dispatch = createEventDispatcher();

  type Step = 'intro' | 'capturing' | 'review';
  let step: Step = 'intro';
  let leaderPositions: number[] = [];
  let followerPositions: number[] = [];
  let offsets: number[] = [];
  let busy = false;

  const JOINT_NAMES = [
    'Shoulder Pan',
    'Shoulder Lift',
    'Elbow Flex',
    'Wrist Flex',
    'Wrist Roll',
    'Gripper',
  ];

  function close() {
    open = false;
    dispatch('close');
  }

  function reset() {
    step = 'intro';
    leaderPositions = [];
    followerPositions = [];
    offsets = [];
  }

  async function capture() {
    busy = true;
    step = 'capturing';
    try {
      leaderPositions = await calibrateCapture('leader');
      followerPositions = await calibrateCapture('follower');
      offsets = await computeCalibration();
      step = 'review';
    } catch (e) {
      showError(`Calibration failed: ${e}`);
      step = 'intro';
    } finally {
      busy = false;
    }
  }

  async function saveAndClose() {
    busy = true;
    try {
      await saveCalibration();
      showStatus('Calibration saved');
      dispatch('saved');
      close();
    } catch (e) {
      showError(`Save failed: ${e}`);
    } finally {
      busy = false;
    }
  }

  async function loadFromFile() {
    busy = true;
    try {
      offsets = await loadCalibration();
      showStatus('Calibration loaded');
      dispatch('saved');
      close();
    } catch (e) {
      showError(`Load failed: ${e}`);
    } finally {
      busy = false;
    }
  }
</script>

{#if open}
  <div class="overlay" on:click|self={close} role="dialog">
    <div class="modal">
      <header class="modal-header">
        <h3>Calibrate Leader / Follower Mirroring</h3>
        <button class="close-btn" on:click={close} aria-label="Close">×</button>
      </header>

      <div class="modal-body">
        {#if step === 'intro'}
          <div class="intro">
            <p>
              Calibration teaches the app how the leader's joint positions
              map to the follower's. Both arms can be in slightly different
              physical orientations even with identical motor IDs, so we
              measure the difference and use it as an offset during mirroring.
            </p>

            <h4>Steps</h4>
            <ol>
              <li>
                Physically move <strong>both arms</strong> into the same reference pose
                (the home position works well: all joints centered, arm pointing up).
              </li>
              <li>
                Hold the arms steady and click <strong>Capture &amp; Compute</strong>.
              </li>
              <li>
                Review the computed offsets, then save to apply.
              </li>
            </ol>

            <div class="actions">
              <button class="btn primary" on:click={capture} disabled={busy}>
                Capture &amp; Compute
              </button>
              <button class="btn" on:click={loadFromFile} disabled={busy}>
                Load saved calibration
              </button>
            </div>
          </div>
        {:else if step === 'capturing'}
          <div class="capturing">
            <p>Reading joint positions from both arms...</p>
          </div>
        {:else if step === 'review'}
          <div class="review">
            <p>
              Calibration complete. The table below shows the position
              difference between leader and follower for each joint. During
              mirroring, the follower will be commanded to
              <em>leader_position + offset</em>.
            </p>

            <table class="offsets-table">
              <thead>
                <tr>
                  <th>Joint</th>
                  <th>Leader</th>
                  <th>Follower</th>
                  <th>Offset</th>
                </tr>
              </thead>
              <tbody>
                {#each JOINT_NAMES as name, i}
                  <tr>
                    <td>{name}</td>
                    <td>{leaderPositions[i] ?? '-'}</td>
                    <td>{followerPositions[i] ?? '-'}</td>
                    <td class:positive={offsets[i] > 0} class:negative={offsets[i] < 0}>
                      {offsets[i] > 0 ? '+' : ''}{offsets[i] ?? 0}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>

            <div class="actions">
              <button class="btn primary" on:click={saveAndClose} disabled={busy}>
                Save calibration
              </button>
              <button class="btn" on:click={reset} disabled={busy}>
                Re-capture
              </button>
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal {
    background: #0f1520;
    border: 1px solid #2a3444;
    border-radius: 8px;
    width: 640px;
    max-width: 90vw;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
    overflow: hidden;
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 18px;
    border-bottom: 1px solid #1e2a3a;
  }

  .modal-header h3 {
    margin: 0;
    color: #c0c8d8;
    font-size: 14px;
    font-weight: 600;
  }

  .close-btn {
    background: none;
    border: none;
    color: #667;
    font-size: 22px;
    cursor: pointer;
    padding: 0 6px;
    line-height: 1;
  }

  .close-btn:hover {
    color: #fff;
  }

  .modal-body {
    padding: 18px;
    overflow-y: auto;
  }

  p {
    color: #aabbcc;
    font-size: 13px;
    line-height: 1.6;
    margin: 0 0 14px;
  }

  h4 {
    color: #c0c8d8;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin: 16px 0 8px;
  }

  ol {
    color: #aabbcc;
    font-size: 13px;
    line-height: 1.7;
    padding-left: 20px;
    margin-bottom: 16px;
  }

  ol strong {
    color: #e0e0e0;
  }

  .actions {
    display: flex;
    gap: 8px;
    margin-top: 12px;
  }

  .btn {
    padding: 8px 16px;
    border: 1px solid #2a3444;
    border-radius: 5px;
    cursor: pointer;
    font-size: 13px;
    background: #0f1520;
    color: #aabbcc;
    transition: all 0.12s;
  }

  .btn:hover:not(:disabled) {
    background: #1a2332;
    color: #c0d0e0;
  }

  .btn.primary {
    background: #1e3a5f;
    color: #60a5fa;
    border-color: #2563eb;
  }

  .btn.primary:hover:not(:disabled) {
    background: #1e4080;
  }

  .btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .capturing {
    text-align: center;
    padding: 20px;
  }

  .offsets-table {
    width: 100%;
    border-collapse: collapse;
    margin: 8px 0;
    font-size: 12px;
    font-family: monospace;
  }

  .offsets-table th {
    text-align: left;
    color: #8899aa;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    padding: 6px 10px;
    border-bottom: 1px solid #1e2a3a;
  }

  .offsets-table td {
    padding: 6px 10px;
    color: #c0d0e0;
    border-bottom: 1px solid #14202e;
  }

  .offsets-table td.positive {
    color: #4ade80;
  }

  .offsets-table td.negative {
    color: #f87171;
  }

  em {
    color: #60a5fa;
    font-style: normal;
    font-family: monospace;
  }
</style>
