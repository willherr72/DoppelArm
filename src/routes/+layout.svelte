<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { refreshCalibrationState } from '$lib/stores/calibration';

  onMount(() => {
    void refreshCalibrationState();
  });
</script>

<div class="app-layout">
  <slot />
</div>

<style>
  .app-layout {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100%;
    overflow: hidden;
  }
</style>
