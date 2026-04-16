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
    height: 100vh;
    width: 100%;
    overflow: hidden;
  }
</style>
