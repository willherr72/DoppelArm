<script lang="ts">
  import { currentMode } from '$lib/stores/app';
  import type { AppMode } from '$lib/stores/app';
  import { bothConnected } from '$lib/stores/connection';
  import { isMirroring } from '$lib/stores/joints';

  const modes: { id: AppMode; label: string; icon: string; requiresConnection: boolean }[] = [
    { id: 'setup', label: 'Setup', icon: '🔌', requiresConnection: false },
    { id: 'calibration', label: 'Calibrate', icon: '📐', requiresConnection: true },
    { id: 'control', label: 'Control', icon: '🎮', requiresConnection: true },
    { id: 'record', label: 'Record', icon: '⏺', requiresConnection: true },
    { id: 'ik', label: 'IK Mode', icon: '🎯', requiresConnection: true },
  ];

  function selectMode(mode: AppMode) {
    if (mode !== 'setup' && !$bothConnected) return;
    currentMode.set(mode);
  }
</script>

<nav class="sidebar">
  <div class="logo">
    <h2>DoppelArm</h2>
  </div>

  <ul class="nav-items">
    {#each modes as mode}
      <li>
        <button
          class="nav-button"
          class:active={$currentMode === mode.id}
          class:disabled={mode.requiresConnection && !$bothConnected}
          on:click={() => selectMode(mode.id)}
        >
          <span class="icon">{mode.icon}</span>
          <span class="label">{mode.label}</span>
        </button>
      </li>
    {/each}
  </ul>

  {#if $isMirroring}
    <div class="mirror-indicator">
      <span class="pulse"></span>
      Mirroring Active
    </div>
  {/if}
</nav>

<style>
  .sidebar {
    width: 200px;
    background: #1a1a2e;
    border-right: 1px solid #333;
    display: flex;
    flex-direction: column;
    padding: 0;
    flex-shrink: 0;
  }

  .logo {
    padding: 16px;
    border-bottom: 1px solid #333;
  }

  .logo h2 {
    margin: 0;
    font-size: 18px;
    color: #e0e0e0;
    font-weight: 600;
  }

  .nav-items {
    list-style: none;
    padding: 8px 0;
    margin: 0;
    flex: 1;
  }

  .nav-button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    background: none;
    border: none;
    color: #aaa;
    font-size: 14px;
    cursor: pointer;
    text-align: left;
    transition: background 0.15s, color 0.15s;
  }

  .nav-button:hover:not(.disabled) {
    background: #16213e;
    color: #fff;
  }

  .nav-button.active {
    background: #0f3460;
    color: #fff;
    border-left: 3px solid #4a90d9;
  }

  .nav-button.disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .icon {
    font-size: 16px;
    width: 24px;
    text-align: center;
  }

  .mirror-indicator {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 16px;
    color: #4ade80;
    font-size: 12px;
    border-top: 1px solid #333;
  }

  .pulse {
    width: 8px;
    height: 8px;
    background: #4ade80;
    border-radius: 50%;
    animation: pulse 1.5s infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }
</style>
