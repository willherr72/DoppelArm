<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import CameraPane from './CameraPane.svelte';

  export let open = false;

  const dispatch = createEventDispatcher();

  function close() {
    open = false;
    dispatch('close');
  }
</script>

{#if open}
  <div class="overlay" on:click|self={close} role="dialog">
    <div class="modal">
      <header class="modal-header">
        <h3>Camera</h3>
        <button class="close-btn" on:click={close} aria-label="Close">×</button>
      </header>
      <div class="modal-body">
        <CameraPane />
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal {
    background: #0f1520;
    border: 1px solid #2a3444;
    border-radius: 8px;
    width: 820px;
    max-width: 92vw;
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
    overflow: hidden;
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 18px;
    border-bottom: 1px solid #1e2a3a;
  }

  .modal-header h3 {
    margin: 0;
    color: #c0c8d8;
    font-size: 14px;
    font-weight: 600;
  }

  .close-btn {
    background: none;
    border: none;
    color: #667;
    font-size: 22px;
    cursor: pointer;
    padding: 0 6px;
    line-height: 1;
  }

  .close-btn:hover {
    color: #fff;
  }

  .modal-body {
    padding: 16px;
    overflow-y: auto;
  }
</style>
