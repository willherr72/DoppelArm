<script lang="ts">
  import { leaderConnection, followerConnection } from '$lib/stores/connection';
  import { error, statusMessage } from '$lib/stores/app';
  import { isMirroring } from '$lib/stores/joints';
  import { isRecording, isPlaying } from '$lib/stores/recording';
</script>

<footer class="status-bar">
  <div class="left">
    <span class="status-item" class:connected={$leaderConnection?.connected}>
      Leader: {$leaderConnection?.connected ? $leaderConnection.port : 'Disconnected'}
    </span>
    <span class="divider">|</span>
    <span class="status-item" class:connected={$followerConnection?.connected}>
      Follower: {$followerConnection?.connected ? $followerConnection.port : 'Disconnected'}
    </span>
  </div>

  <div class="center">
    {#if $error}
      <span class="error">{$error}</span>
    {:else if $statusMessage}
      <span class="message">{$statusMessage}</span>
    {/if}
  </div>

  <div class="right">
    {#if $isRecording}
      <span class="recording">REC</span>
    {/if}
    {#if $isPlaying}
      <span class="playing">PLAY</span>
    {/if}
    {#if $isMirroring}
      <span class="mirroring">MIRROR</span>
    {/if}
  </div>
</footer>

<style>
  .status-bar {
    height: 28px;
    background: #1a1a2e;
    border-top: 1px solid #333;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    font-size: 12px;
    color: #888;
    flex-shrink: 0;
  }

  .left, .center, .right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .divider {
    color: #444;
  }

  .status-item.connected {
    color: #4ade80;
  }

  .error {
    color: #f87171;
  }

  .message {
    color: #60a5fa;
  }

  .recording {
    color: #f87171;
    font-weight: bold;
    animation: pulse 1s infinite;
  }

  .playing {
    color: #4ade80;
    font-weight: bold;
  }

  .mirroring {
    color: #facc15;
    font-weight: bold;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }
</style>
