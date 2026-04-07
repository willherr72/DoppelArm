<script lang="ts">
  import Toolbar from '$lib/components/Toolbar.svelte';
  import ArmPane from '$lib/components/ArmPane.svelte';
  import { ARM_COLORS } from '$lib/utils/arm-config';
</script>

<Toolbar />

<div class="split-view">
  <div class="pane leader-pane">
    <ArmPane role="leader" color={ARM_COLORS.leader} />
  </div>
  <div class="divider"></div>
  <div class="pane follower-pane">
    <ArmPane role="follower" color={ARM_COLORS.follower} />
  </div>
</div>

<style>
  .split-view {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  .pane {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .divider {
    width: 1px;
    background: #1e2a3a;
    flex-shrink: 0;
  }
</style>
