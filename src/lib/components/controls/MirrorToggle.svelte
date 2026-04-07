<script lang="ts">
  import { isMirroring, leaderJoints, followerJoints } from '$lib/stores/joints';
  import { startMirroring, stopMirroring } from '$lib/tauri/commands';
  import { showError, showStatus } from '$lib/stores/app';

  let starting = false;

  async function toggleMirror() {
    if ($isMirroring) {
      try {
        await stopMirroring();
        isMirroring.set(false);
        showStatus('Mirroring stopped');
      } catch (e) {
        showError(`Stop failed: ${e}`);
      }
    } else {
      starting = true;
      try {
        await startMirroring((payload) => {
          leaderJoints.set({
            positions: payload.leader,
            timestamp: payload.timestamp_ms,
          });
          followerJoints.set({
            positions: payload.follower,
            timestamp: payload.timestamp_ms,
          });
        });
        isMirroring.set(true);
        showStatus('Mirroring started');
      } catch (e) {
        showError(`Start failed: ${e}`);
      } finally {
        starting = false;
      }
    }
  }
</script>

<button
  class="mirror-btn"
  class:active={$isMirroring}
  on:click={toggleMirror}
  disabled={starting}
>
  {#if starting}
    Starting...
  {:else if $isMirroring}
    Stop Mirroring
  {:else}
    Start Mirroring
  {/if}
</button>

<style>
  .mirror-btn {
    width: 100%;
    padding: 12px;
    border: 2px solid #4a90d9;
    border-radius: 8px;
    background: transparent;
    color: #4a90d9;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .mirror-btn:hover:not(:disabled) {
    background: #4a90d920;
  }

  .mirror-btn.active {
    background: #4a90d9;
    color: white;
    animation: glow 2s infinite;
  }

  .mirror-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  @keyframes glow {
    0%, 100% { box-shadow: 0 0 5px #4a90d950; }
    50% { box-shadow: 0 0 15px #4a90d980; }
  }
</style>
