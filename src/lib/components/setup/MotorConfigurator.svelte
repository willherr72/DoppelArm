<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { configureMotor, autoDetectMotor, scanMotors } from '$lib/tauri/commands';
  import { showError, showStatus } from '$lib/stores/app';

  export let port: string;
  export let armLabel: string;

  const dispatch = createEventDispatcher();

  const JOINT_NAMES = [
    'Shoulder Pan',
    'Shoulder Lift',
    'Elbow Flex',
    'Wrist Flex',
    'Wrist Roll',
    'Gripper',
  ];

  let currentJoint = 0;
  let configuring = false;
  let verifying = false;
  let configured: boolean[] = Array(6).fill(false);
  let verifyResult: { ok: boolean; found: number[] } | null = null;

  $: allConfigured = configured.every(Boolean);

  async function configureCurrentMotor() {
    configuring = true;
    try {
      const [detectedId, _detectedBaud] = await autoDetectMotor(port);
      showStatus(`Found motor at ID ${detectedId}, setting to ID ${currentJoint + 1}...`);

      await configureMotor(port, detectedId, currentJoint + 1);
      configured[currentJoint] = true;
      showStatus(`${JOINT_NAMES[currentJoint]} configured as ID ${currentJoint + 1}`);

      // Advance to next joint automatically
      if (currentJoint < 5) {
        currentJoint++;
      }
    } catch (e) {
      showError(`Configuration failed: ${e}`);
    } finally {
      configuring = false;
    }
  }

  async function verifyAll() {
    verifying = true;
    verifyResult = null;
    try {
      const ids = await scanMotors(port);
      const expected = [1, 2, 3, 4, 5, 6];
      const allPresent = expected.every((id) => ids.includes(id));
      verifyResult = { ok: allPresent, found: ids };
      if (allPresent) {
        showStatus(`All 6 motors verified on ${armLabel} arm`);
      } else {
        const missing = expected.filter((id) => !ids.includes(id));
        showError(`Missing IDs: ${missing.join(', ')}. Found: ${ids.join(', ') || 'none'}`);
      }
    } catch (e) {
      showError(`Verification failed: ${e}`);
    } finally {
      verifying = false;
    }
  }

  function finish() {
    dispatch('done');
  }

  function jumpToJoint(i: number) {
    if (!configuring) currentJoint = i;
  }

  function resetAll() {
    configured = Array(6).fill(false);
    currentJoint = 0;
    verifyResult = null;
  }
</script>

<div class="configurator">
  <div class="motor-list">
    {#each JOINT_NAMES as name, i}
      <button
        class="motor-item"
        class:active={currentJoint === i}
        class:done={configured[i]}
        on:click={() => jumpToJoint(i)}
        disabled={configuring}
      >
        <span class="motor-id">ID {i + 1}</span>
        <span class="motor-name">{name}</span>
        {#if configured[i]}
          <span class="check">OK</span>
        {:else if currentJoint === i}
          <span class="current">current</span>
        {/if}
      </button>
    {/each}
  </div>

  <div class="instructions">
    {#if !allConfigured}
      <p>
        Connect <strong>only the {JOINT_NAMES[currentJoint]}</strong> motor to the controller board.
        Make sure no other motors are connected, then click Configure.
      </p>

      <div class="actions">
        <button class="btn primary" on:click={configureCurrentMotor} disabled={configuring}>
          {configuring ? 'Configuring...' : `Configure as ID ${currentJoint + 1}`}
        </button>

        <button class="btn" on:click={verifyAll} disabled={verifying}>
          {verifying ? 'Scanning bus...' : 'Verify what\'s on the bus'}
        </button>

        {#if verifyResult}
          <div class="verify-result" class:ok={verifyResult.ok} class:fail={!verifyResult.ok}>
            {#if verifyResult.found.length === 0}
              No motors found on the bus
            {:else}
              Found IDs: {verifyResult.found.join(', ')}
            {/if}
          </div>
        {/if}

        <div class="spacer"></div>

        <button class="btn secondary" on:click={finish} disabled={configuring}>
          Exit setup
        </button>
      </div>
    {:else}
      <div class="done-state">
        <p class="done-msg">
          <strong>All 6 motors configured.</strong>
          Reconnect the full daisy chain (motor 1 to motor 2 to motor 3... etc.)
          then click Verify to confirm every motor responds on the bus.
        </p>

        <div class="actions">
          <button class="btn primary" on:click={verifyAll} disabled={verifying}>
            {verifying ? 'Scanning bus...' : 'Verify all motors'}
          </button>

          {#if verifyResult}
            <div class="verify-result" class:ok={verifyResult.ok} class:fail={!verifyResult.ok}>
              {#if verifyResult.ok}
                All 6 motors found ({verifyResult.found.join(', ')})
              {:else}
                Found: {verifyResult.found.length ? verifyResult.found.join(', ') : 'none'}
                (expected 1, 2, 3, 4, 5, 6)
              {/if}
            </div>
          {/if}

          {#if verifyResult?.ok}
            <button class="btn primary" on:click={finish}>
              Finish setup
            </button>
          {/if}

          <button class="btn" on:click={resetAll}>
            Restart setup
          </button>

          <div class="spacer"></div>

          <button class="btn secondary" on:click={finish}>
            Exit setup
          </button>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .configurator {
    display: flex;
    gap: 20px;
  }

  .motor-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 200px;
  }

  .motor-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-radius: 5px;
    background: #0a0e14;
    border: 1px solid transparent;
    color: #778899;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
    transition: all 0.12s;
  }

  .motor-item:hover:not(:disabled) {
    background: #111827;
  }

  .motor-item.active {
    background: #111827;
    color: #c0d0e0;
    border-color: #2563eb;
  }

  .motor-item.done {
    color: #4ade80;
  }

  .motor-item:disabled {
    cursor: not-allowed;
    opacity: 0.6;
  }

  .motor-id {
    font-weight: 700;
    min-width: 35px;
    font-family: monospace;
  }

  .motor-name {
    flex: 1;
  }

  .check {
    font-size: 10px;
    color: #4ade80;
    font-weight: 700;
  }

  .current {
    font-size: 10px;
    color: #facc15;
  }

  .instructions {
    flex: 1;
  }

  .instructions p {
    color: #aabbcc;
    margin-bottom: 14px;
    font-size: 13px;
    line-height: 1.5;
  }

  .instructions strong {
    color: #e0e0e0;
  }

  .done-msg {
    padding: 10px 12px;
    background: #0a1a12;
    border-left: 3px solid #4ade80;
    border-radius: 4px;
  }

  .actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .spacer {
    height: 6px;
    border-top: 1px solid #1e2a3a;
    margin: 4px 0;
  }

  .verify-result {
    padding: 8px 12px;
    border-radius: 4px;
    font-size: 12px;
    font-family: monospace;
  }

  .verify-result.ok {
    background: #0d3320;
    color: #4ade80;
    border: 1px solid #166534;
  }

  .verify-result.fail {
    background: #2a0f10;
    color: #f87171;
    border: 1px solid #7f1d1d;
  }

  .btn {
    padding: 8px 16px;
    border: 1px solid #2a3444;
    border-radius: 5px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 500;
    transition: all 0.12s;
    background: #0f1520;
    color: #aabbcc;
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

  .btn.secondary {
    background: #111827;
    color: #8899aa;
    border-color: #1e2a3a;
  }

  .btn.secondary:hover:not(:disabled) {
    background: #1a2332;
    color: #c0d0e0;
  }

  .btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
