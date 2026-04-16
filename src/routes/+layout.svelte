<script lang="ts">
  import '../app.css';
</script>

<div class="app-layout">
  <slot />
</div>

<style>
  .app-layout {
    display: flex;
    flex-direction: column;
    min-height: 100vh;
    width: 100%;
    overflow-x: hidden;
    overflow-y: auto;
  }
</style>
