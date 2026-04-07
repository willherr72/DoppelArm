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
  let configured: boolean[] = Array(6).fill(false);

  async function configureCurrentMotor() {
    configuring = true;
    try {
      // Try to auto-detect the motor first
      const [detectedId, detectedBaud] = await autoDetectMotor(port);
      showStatus(`Found motor at ID ${detectedId}, configuring as ID ${currentJoint + 1}...`);

      // Configure the motor with the target ID
      await configureMotor(port, detectedId, currentJoint + 1);
      configured[currentJoint] = true;
      showStatus(`${JOINT_NAMES[currentJoint]} configured as ID ${currentJoint + 1}`);

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
    try {
      const ids = await scanMotors(port);
      const allPresent = [1, 2, 3, 4, 5, 6].every(id => ids.includes(id));
      if (allPresent) {
        showStatus(`All 6 motors verified on ${armLabel} arm!`);
        dispatch('done');
      } else {
        showError(`Found motors: [${ids.join(', ')}]. Expected [1,2,3,4,5,6].`);
      }
    } catch (e) {
      showError(`Verification failed: ${e}`);
    }
  }

  function skipToVerify() {
    dispatch('done');
  }
</script>

<div class="configurator">
  <div class="motor-list">
    {#each JOINT_NAMES as name, i}
      <div class="motor-item" class:active={currentJoint === i} class:done={configured[i]}>
        <span class="motor-id">ID {i + 1}</span>
        <span class="motor-name">{name}</span>
        {#if configured[i]}
          <span class="check">Done</span>
        {:else if currentJoint === i}
          <span class="current">Current</span>
        {/if}
      </div>
    {/each}
  </div>

  <div class="instructions">
    <p>
      Connect <strong>only the {JOINT_NAMES[currentJoint]}</strong> motor to the controller board.
      Make sure no other motors are connected, then click Configure.
    </p>

    <div class="actions">
      <button class="btn primary" on:click={configureCurrentMotor} disabled={configuring}>
        {configuring ? 'Configuring...' : `Configure as ID ${currentJoint + 1}`}
      </button>

      {#if configured.every(Boolean)}
        <button class="btn primary" on:click={verifyAll}>
          Verify All Motors &rarr;
        </button>
      {/if}

      <button class="btn secondary" on:click={skipToVerify}>
        Skip (already configured)
      </button>
    </div>
  </div>
</div>

<style>
  .configurator {
    display: flex;
    gap: 24px;
  }

  .motor-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 200px;
  }

  .motor-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-radius: 6px;
    background: #1a1a2e;
    color: #888;
    font-size: 13px;
  }

  .motor-item.active {
    background: #16213e;
    color: #e0e0e0;
    border: 1px solid #4a90d9;
  }

  .motor-item.done {
    color: #4ade80;
  }

  .motor-id {
    font-weight: bold;
    min-width: 35px;
  }

  .check {
    margin-left: auto;
    color: #4ade80;
    font-size: 11px;
  }

  .current {
    margin-left: auto;
    color: #facc15;
    font-size: 11px;
  }

  .instructions {
    flex: 1;
  }

  .instructions p {
    color: #ccc;
    margin-bottom: 16px;
  }

  .actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .btn {
    padding: 8px 16px;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
  }

  .btn.primary {
    background: #4a90d9;
    color: white;
  }

  .btn.primary:hover:not(:disabled) {
    background: #3a7bc8;
  }

  .btn.secondary {
    background: #333;
    color: #ccc;
  }

  .btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
