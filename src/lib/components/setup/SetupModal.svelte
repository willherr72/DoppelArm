<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import MotorConfigurator from './MotorConfigurator.svelte';

  export let port: string;
  export let armLabel: string;
  export let open: boolean = false;

  const dispatch = createEventDispatcher();

  function close() {
    open = false;
    dispatch('close');
  }

  function onDone() {
    close();
    dispatch('done');
  }
</script>

{#if open}
  <div class="overlay" on:click|self={close} role="dialog">
    <div class="modal">
      <header class="modal-header">
        <h3>Configure {armLabel} Motors — {port}</h3>
        <button class="close-btn" on:click={close} aria-label="Close">×</button>
      </header>

      <div class="instructions-banner">
        <strong>Important:</strong> Disconnect the daisy chain and connect <em>only one motor at a time</em>
        to the controller board. Each motor will be assigned a unique ID 1-6.
      </div>

      <div class="modal-body">
        <MotorConfigurator {port} {armLabel} on:done={onDone} />
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
    width: 720px;
    max-width: 90vw;
    max-height: 85vh;
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

  .instructions-banner {
    padding: 10px 18px;
    background: #1a2332;
    border-bottom: 1px solid #1e2a3a;
    color: #aab;
    font-size: 12px;
    line-height: 1.5;
  }

  .instructions-banner strong {
    color: #facc15;
  }

  .instructions-banner em {
    color: #60a5fa;
    font-style: normal;
    font-weight: 600;
  }

  .modal-body {
    padding: 18px;
    overflow-y: auto;
  }
</style>
