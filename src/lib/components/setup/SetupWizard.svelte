<script lang="ts">
  import { availablePorts, leaderConnection, followerConnection } from '$lib/stores/connection';
  import { currentMode, showError, showStatus } from '$lib/stores/app';
  import { listPorts, connectArm, disconnectArm, scanMotors, configureMotor, autoDetectMotor } from '$lib/tauri/commands';
  import PortSelector from './PortSelector.svelte';
  import MotorConfigurator from './MotorConfigurator.svelte';

  let step: 'ports' | 'leader-config' | 'follower-config' | 'done' = 'ports';
  let leaderPort = '';
  let followerPort = '';
  let scanning = false;

  async function refreshPorts() {
    try {
      const ports = await listPorts();
      availablePorts.set(ports);
    } catch (e) {
      showError(`Failed to list ports: ${e}`);
    }
  }

  async function connectLeader() {
    if (!leaderPort) return;
    try {
      await connectArm(leaderPort, 'leader');
      leaderConnection.set({ port: leaderPort, baudRate: 1000000, connected: true, motorIds: [] });
      showStatus('Leader arm connected');

      // Try to scan for motors
      try {
        const ids = await scanMotors(leaderPort);
        leaderConnection.update(c => c ? { ...c, motorIds: ids } : c);
      } catch { /* motors may not be configured yet */ }
    } catch (e) {
      showError(`Failed to connect leader: ${e}`);
    }
  }

  async function connectFollower() {
    if (!followerPort) return;
    try {
      await connectArm(followerPort, 'follower');
      followerConnection.set({ port: followerPort, baudRate: 1000000, connected: true, motorIds: [] });
      showStatus('Follower arm connected');

      try {
        const ids = await scanMotors(followerPort);
        followerConnection.update(c => c ? { ...c, motorIds: ids } : c);
      } catch { /* motors may not be configured yet */ }
    } catch (e) {
      showError(`Failed to connect follower: ${e}`);
    }
  }

  function proceed() {
    if (step === 'ports') {
      step = 'leader-config';
    } else if (step === 'leader-config') {
      step = 'follower-config';
    } else if (step === 'follower-config') {
      step = 'done';
      currentMode.set('control');
    }
  }

  // Refresh ports on mount
  refreshPorts();
</script>

<div class="wizard">
  <h2>Setup Wizard</h2>

  {#if step === 'ports'}
    <div class="step">
      <h3>Step 1: Connect Arms</h3>
      <p class="description">
        Connect both USB serial adapters. Select the port for each arm.
      </p>

      <button class="btn secondary" on:click={refreshPorts}>Refresh Ports</button>

      <div class="port-grid">
        <div class="port-section">
          <h4>Leader Arm</h4>
          <PortSelector
            ports={$availablePorts}
            bind:selectedPort={leaderPort}
            disabledPort={followerPort}
          />
          <button class="btn primary" on:click={connectLeader} disabled={!leaderPort}>
            Connect Leader
          </button>
          {#if $leaderConnection?.connected}
            <span class="connected-badge">Connected ({$leaderConnection.motorIds.length} motors found)</span>
          {/if}
        </div>

        <div class="port-section">
          <h4>Follower Arm</h4>
          <PortSelector
            ports={$availablePorts}
            bind:selectedPort={followerPort}
            disabledPort={leaderPort}
          />
          <button class="btn primary" on:click={connectFollower} disabled={!followerPort}>
            Connect Follower
          </button>
          {#if $followerConnection?.connected}
            <span class="connected-badge">Connected ({$followerConnection.motorIds.length} motors found)</span>
          {/if}
        </div>
      </div>

      {#if $leaderConnection?.connected && $followerConnection?.connected}
        <button class="btn primary large" on:click={proceed}>
          Next: Configure Motors &rarr;
        </button>
      {/if}
    </div>

  {:else if step === 'leader-config'}
    <div class="step">
      <h3>Step 2: Configure Leader Motors</h3>
      <p class="description">
        Connect each motor one at a time to the leader controller board and assign IDs 1-6.
      </p>
      <MotorConfigurator port={leaderPort} armLabel="Leader" on:done={proceed} />
    </div>

  {:else if step === 'follower-config'}
    <div class="step">
      <h3>Step 3: Configure Follower Motors</h3>
      <p class="description">
        Connect each motor one at a time to the follower controller board and assign IDs 1-6.
      </p>
      <MotorConfigurator port={followerPort} armLabel="Follower" on:done={proceed} />
    </div>

  {:else if step === 'done'}
    <div class="step">
      <h3>Setup Complete!</h3>
      <p>Both arms are configured. You can now calibrate and control the arms.</p>
      <button class="btn primary" on:click={() => currentMode.set('calibration')}>
        Proceed to Calibration &rarr;
      </button>
    </div>
  {/if}
</div>

<style>
  .wizard {
    padding: 24px;
    max-width: 700px;
  }

  h2 {
    margin: 0 0 20px;
    color: #e0e0e0;
  }

  h3 {
    color: #c0c0c0;
    margin: 0 0 8px;
  }

  h4 {
    color: #a0a0a0;
    margin: 0 0 8px;
  }

  .description {
    color: #888;
    margin-bottom: 16px;
  }

  .step {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .port-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 24px;
    margin: 16px 0;
  }

  .port-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 16px;
    background: #16213e;
    border-radius: 8px;
    border: 1px solid #333;
  }

  .connected-badge {
    color: #4ade80;
    font-size: 12px;
  }

  .btn {
    padding: 8px 16px;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
    transition: background 0.15s;
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

  .btn.secondary:hover {
    background: #444;
  }

  .btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .btn.large {
    padding: 12px 24px;
    font-size: 15px;
    margin-top: 12px;
  }
</style>
