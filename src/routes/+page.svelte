<script lang="ts">
  import Toolbar from '$lib/components/Toolbar.svelte';
  import ArmPane from '$lib/components/ArmPane.svelte';
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
</div>

<style>
  .layout {
    flex: 1;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    padding: 10px;
    min-height: 0;
  }

  .pane {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  @media (max-width: 1100px) {
    .layout {
      grid-template-columns: 1fr;
    }
  }
</style>
