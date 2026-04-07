<script lang="ts">
  import type { PortInfo } from '$lib/stores/connection';

  export let ports: PortInfo[] = [];
  export let selectedPort: string = '';
  export let disabledPort: string = '';
</script>

<select bind:value={selectedPort}>
  <option value="">-- Select Port --</option>
  {#each ports as port}
    <option value={port.name} disabled={port.name === disabledPort}>
      {port.name} {port.description ? `(${port.description})` : ''}
    </option>
  {/each}
</select>

<style>
  select {
    width: 100%;
    padding: 8px 12px;
    background: #1a1a2e;
    color: #e0e0e0;
    border: 1px solid #444;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
  }

  select:focus {
    outline: none;
    border-color: #4a90d9;
  }

  option:disabled {
    color: #666;
  }
</style>
