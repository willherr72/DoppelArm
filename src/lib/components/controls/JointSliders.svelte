<script lang="ts">
  import { leaderJoints, followerJoints, JOINT_NAMES, JOINT_LIMITS, rawToDegrees, isMirroring } from '$lib/stores/joints';
  import { leaderConnection, followerConnection } from '$lib/stores/connection';
  import { readAllJoints, writeSingleJoint } from '$lib/tauri/commands';
  import { showError } from '$lib/stores/app';

  export let role: 'leader' | 'follower' = 'follower';
  export let readonly: boolean = false;

  $: joints = role === 'leader' ? $leaderJoints : $followerJoints;
  $: positions = joints.positions;
  $: isDisabled = readonly || $isMirroring;
  $: connected = role === 'leader' ? $leaderConnection?.connected : $followerConnection?.connected;

  let refreshing = false;

  async function refresh() {
    if (!connected) return;
    refreshing = true;
    try {
      const pos = await readAllJoints(role);
      const store = role === 'leader' ? leaderJoints : followerJoints;
      store.update(s => ({ ...s, positions: pos }));
    } catch (e) {
      showError(`Read failed: ${e}`);
    } finally {
      refreshing = false;
    }
  }

  function onSliderChange(jointIndex: number, value: number) {
    if (readonly) return;

    // Always update the store so the 3D model follows
    const store = role === 'leader' ? leaderJoints : followerJoints;
    store.update(s => {
      const newPos = [...s.positions];
      newPos[jointIndex] = value;
      return { ...s, positions: newPos };
    });

    // Send to hardware if connected
    if (connected) {
      writeSingleJoint(role, jointIndex, value).catch(() => {});
    }
  }
</script>

<div class="sliders" class:readonly>
  <div class="header">
    <span class="title">Joints {readonly ? '(read-only)' : ''}</span>
    <button class="btn-sm" on:click={refresh} disabled={refreshing}>
      {refreshing ? '...' : 'Read'}
    </button>
  </div>

  {#each JOINT_NAMES as name, i}
    <div class="slider-row">
      <div class="joint-label">
        <span class="name">{name}</span>
        <span class="value">{rawToDegrees(positions[i]).toFixed(1)}</span>
      </div>
      {#if readonly}
        <div class="bar-track">
          <div
            class="bar-fill"
            style="width: {((positions[i] - JOINT_LIMITS[i][0]) / (JOINT_LIMITS[i][1] - JOINT_LIMITS[i][0])) * 100}%"
          ></div>
        </div>
      {:else}
        <input
          type="range"
          min={JOINT_LIMITS[i][0]}
          max={JOINT_LIMITS[i][1]}
          value={positions[i]}
          disabled={isDisabled}
          on:input={(e) => onSliderChange(i, parseInt(e.currentTarget.value))}
        />
      {/if}
    </div>
  {/each}
</div>

<style>
  .sliders {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 2px;
  }

  .title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #556677;
  }

  .slider-row {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .joint-label {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
  }

  .name { color: #778899; }

  .value {
    color: #556677;
    font-family: monospace;
    font-size: 10px;
  }

  /* Read-only bar (leader) */
  .bar-track {
    width: 100%;
    height: 3px;
    background: #1a2332;
    border-radius: 2px;
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    background: #2a3a4a;
    border-radius: 2px;
    transition: width 0.05s linear;
  }

  .readonly .bar-fill {
    background: #2a4a3a;
  }

  /* Interactive slider (follower) */
  input[type="range"] {
    width: 100%;
    height: 3px;
    -webkit-appearance: none;
    appearance: none;
    background: #1a2332;
    border-radius: 2px;
    outline: none;
  }

  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #3a5a7a;
    cursor: pointer;
    transition: background 0.1s;
  }

  input[type="range"]::-webkit-slider-thumb:hover {
    background: #4a7aaa;
  }

  input[type="range"]:disabled::-webkit-slider-thumb {
    background: #222;
    cursor: not-allowed;
  }

  .btn-sm {
    padding: 2px 8px;
    background: #111827;
    color: #556677;
    border: 1px solid #1e2a3a;
    border-radius: 3px;
    font-size: 10px;
    cursor: pointer;
  }

  .btn-sm:hover:not(:disabled) {
    color: #8899aa;
    border-color: #2a3a4a;
  }
</style>
