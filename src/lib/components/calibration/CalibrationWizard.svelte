<script lang="ts">
  import { calibrateCapture, computeCalibration, saveCalibration, loadCalibration } from '$lib/tauri/commands';
  import { showError, showStatus, currentMode } from '$lib/stores/app';
  import { JOINT_NAMES } from '$lib/stores/joints';

  let step: 'instructions' | 'capture-leader' | 'capture-follower' | 'done' = 'instructions';
  let leaderPositions: number[] = [];
  let followerPositions: number[] = [];
  let offsets: number[] = [];
  let capturing = false;

  async function captureLeader() {
    capturing = true;
    try {
      leaderPositions = await calibrateCapture('leader');
      showStatus('Leader reference captured');
      step = 'capture-follower';
    } catch (e) {
      showError(`Capture failed: ${e}`);
    } finally {
      capturing = false;
    }
  }

  async function captureFollower() {
    capturing = true;
    try {
      followerPositions = await calibrateCapture('follower');
      showStatus('Follower reference captured');
      offsets = await computeCalibration();
      step = 'done';
    } catch (e) {
      showError(`Capture failed: ${e}`);
    } finally {
      capturing = false;
    }
  }

  async function save() {
    try {
      await saveCalibration();
      showStatus('Calibration saved');
    } catch (e) {
      showError(`Save failed: ${e}`);
    }
  }

  async function load() {
    try {
      offsets = await loadCalibration();
      step = 'done';
      showStatus('Calibration loaded');
    } catch (e) {
      showError(`Load failed: ${e}`);
    }
  }
</script>

<div class="wizard">
  <h2>Calibration</h2>

  {#if step === 'instructions'}
    <div class="step">
      <h3>Align Both Arms</h3>
      <p class="description">
        Physically move both the leader and follower arms to the <strong>same reference position</strong>.
        A good reference is having all joints pointing straight up (home position).
        This ensures the leader-follower mirroring is accurate.
      </p>
      <div class="actions">
        <button class="btn primary" on:click={() => step = 'capture-leader'}>
          Begin Calibration
        </button>
        <button class="btn secondary" on:click={load}>
          Load Previous Calibration
        </button>
      </div>
    </div>

  {:else if step === 'capture-leader'}
    <div class="step">
      <h3>Capture Leader Position</h3>
      <p class="description">
        Ensure the leader arm is in the reference position, then click Capture.
      </p>
      <button class="btn primary" on:click={captureLeader} disabled={capturing}>
        {capturing ? 'Capturing...' : 'Capture Leader'}
      </button>
    </div>

  {:else if step === 'capture-follower'}
    <div class="step">
      <h3>Capture Follower Position</h3>
      <p class="description">
        Now ensure the follower arm is in the <strong>same</strong> reference position, then click Capture.
      </p>

      {#if leaderPositions.length}
        <div class="positions">
          <h4>Leader Reference:</h4>
          {#each leaderPositions as pos, i}
            <span class="pos-item">{JOINT_NAMES[i]}: {pos}</span>
          {/each}
        </div>
      {/if}

      <button class="btn primary" on:click={captureFollower} disabled={capturing}>
        {capturing ? 'Capturing...' : 'Capture Follower'}
      </button>
    </div>

  {:else if step === 'done'}
    <div class="step">
      <h3>Calibration Complete</h3>

      <div class="offsets">
        <h4>Computed Offsets:</h4>
        {#each offsets as offset, i}
          <div class="offset-item">
            <span class="joint-name">{JOINT_NAMES[i]}</span>
            <span class="offset-value" class:positive={offset > 0} class:negative={offset < 0}>
              {offset > 0 ? '+' : ''}{offset}
            </span>
          </div>
        {/each}
      </div>

      <div class="actions">
        <button class="btn primary" on:click={save}>Save Calibration</button>
        <button class="btn secondary" on:click={() => step = 'instructions'}>Recalibrate</button>
        <button class="btn primary" on:click={() => currentMode.set('control')}>
          Proceed to Control &rarr;
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  .wizard {
    padding: 24px;
    max-width: 600px;
  }

  h2 { margin: 0 0 20px; color: #e0e0e0; }
  h3 { color: #c0c0c0; margin: 0 0 8px; }
  h4 { color: #a0a0a0; margin: 8px 0; }

  .description { color: #888; margin-bottom: 16px; }

  .step {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .actions {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }

  .positions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 12px;
    background: #16213e;
    border-radius: 6px;
  }

  .pos-item {
    font-size: 12px;
    color: #aaa;
    padding: 2px 8px;
    background: #1a1a2e;
    border-radius: 4px;
  }

  .offsets {
    padding: 16px;
    background: #16213e;
    border-radius: 8px;
  }

  .offset-item {
    display: flex;
    justify-content: space-between;
    padding: 4px 0;
    font-size: 13px;
  }

  .joint-name { color: #ccc; }
  .offset-value { font-family: monospace; color: #888; }
  .offset-value.positive { color: #4ade80; }
  .offset-value.negative { color: #f87171; }

  .btn {
    padding: 8px 16px;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
  }

  .btn.primary { background: #4a90d9; color: white; }
  .btn.primary:hover:not(:disabled) { background: #3a7bc8; }
  .btn.secondary { background: #333; color: #ccc; }
  .btn.secondary:hover { background: #444; }
  .btn:disabled { opacity: 0.4; cursor: not-allowed; }
</style>
