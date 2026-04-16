<script lang="ts">
  import Toolbar from '$lib/components/Toolbar.svelte';
  import ArmPane from '$lib/components/ArmPane.svelte';
  import CameraPane from '$lib/components/CameraPane.svelte';
  import { ARM_COLORS } from '$lib/utils/arm-config';
</script>

<Toolbar />

<div class="layout">
  <div class="pane">
    <ArmPane role="leader" color={ARM_COLORS.leader} />
  </div>
  <div class="pane">
    <ArmPane role="follower" color={ARM_COLORS.follower} />
  </div>
  <div class="camera-slot">
    <CameraPane />
  </div>
</div>

<style>
  .layout {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    align-content: start;
    padding: 10px;
    min-height: 100%;
  }

  .pane {
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .camera-slot {
    grid-column: 1 / -1;
    min-width: 0;
  }

  @media (max-width: 1100px) {
    .layout {
      grid-template-columns: 1fr;
    }
  }
</style>
