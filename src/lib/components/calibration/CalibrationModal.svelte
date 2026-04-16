<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import {
    calibrateCapture,
    computeCalibration,
    saveCalibration,
    loadCalibration,
    hasSavedCalibration,
    getCalibrationState,
    setCalibrationState,
    setMirrorSign
  } from '$lib/tauri/commands';
  import type { CalibrationState } from '$lib/tauri/commands';
  import { showError, showStatus } from '$lib/stores/app';

  export let open = false;

  const dispatch = createEventDispatcher();

  type Step = 'intro' | 'capturing' | 'review';
  type DraftSource = 'session' | 'capture' | 'saved';

  const JOINT_NAMES = [
    'Shoulder Pan',
    'Shoulder Lift',
    'Elbow Flex',
    'Wrist Flex',
    'Wrist Roll',
    'Gripper'
  ];

  let step: Step = 'intro';
  let busy = false;
  let wasOpen = false;
  let savedExists = false;
  let initialSession: CalibrationState | null = null;
  let draftSource: DraftSource = 'session';
  let keepSessionChanges = false;
  let hasDraftChanges = false;

  let leaderPositions: number[] = [];
  let followerPositions: number[] = [];
  let offsets: number[] = [];
  let mirrorSigns: number[] = [1, 1, 1, 1, 1, 1];

  function calibrationLabel(source: DraftSource) {
    if (source === 'capture') return 'new capture';
    if (source === 'saved') return 'saved calibration';
    return 'current session';
  }

  function applyDraft(calibration: CalibrationState) {
    leaderPositions = calibration.leader_reference;
    followerPositions = calibration.follower_reference;
    offsets = calibration.offsets;
    mirrorSigns = calibration.mirror_signs;
  }

  async function initializeModal() {
    busy = true;
    step = 'intro';
    keepSessionChanges = false;
    hasDraftChanges = false;

    try {
      savedExists = await hasSavedCalibration();
      initialSession = await getCalibrationState();
      applyDraft(initialSession);
      draftSource = 'session';
    } catch (e) {
      showError(`Could not initialize calibration modal: ${e}`);
    } finally {
      busy = false;
    }
  }

  async function restoreInitialSession() {
    if (!initialSession) return;
    try {
      await setCalibrationState(initialSession);
      applyDraft(initialSession);
    } catch (e) {
      showError(`Could not restore previous calibration: ${e}`);
    }
  }

  async function closeModal() {
    if (hasDraftChanges && !keepSessionChanges) {
      await restoreInitialSession();
      showStatus('Discarded draft calibration changes');
    }
    open = false;
    dispatch('close');
  }

  function close() {
    void closeModal();
  }

  async function reviewCurrentSession() {
    busy = true;
    try {
      const current = await getCalibrationState();
      applyDraft(current);
      draftSource = 'session';
      hasDraftChanges = false;
      step = 'review';
      showStatus('Loaded current session calibration');
    } catch (e) {
      showError(`Could not read current calibration: ${e}`);
    } finally {
      busy = false;
    }
  }

  async function captureNewCalibration() {
    busy = true;
    step = 'capturing';
    try {
      await calibrateCapture('leader');
      await calibrateCapture('follower');
      await computeCalibration();
      const current = await getCalibrationState();
      applyDraft(current);
      draftSource = 'capture';
      hasDraftChanges = true;
      step = 'review';
      showStatus('Captured new calibration draft');
    } catch (e) {
      showError(`Calibration failed: ${e}`);
      await restoreInitialSession();
      step = 'intro';
    } finally {
      busy = false;
    }
  }

  async function loadSavedIntoDraft() {
    busy = true;
    try {
      await loadCalibration();
      const loaded = await getCalibrationState();
      applyDraft(loaded);
      draftSource = 'saved';
      hasDraftChanges = true;
      step = 'review';
      showStatus('Loaded saved calibration into draft');
    } catch (e) {
      showError(`Load failed: ${e}`);
    } finally {
      busy = false;
    }
  }

  async function saveAsDefault() {
    busy = true;
    try {
      await saveCalibration();
      initialSession = await getCalibrationState();
      savedExists = true;
      keepSessionChanges = true;
      hasDraftChanges = false;
      showStatus('Calibration saved as default');
      dispatch('saved');
      open = false;
      dispatch('close');
    } catch (e) {
      showError(`Save failed: ${e}`);
    } finally {
      busy = false;
    }
  }

  async function applyForSession() {
    try {
      initialSession = await getCalibrationState();
      keepSessionChanges = true;
      hasDraftChanges = false;
      showStatus('Calibration applied for this session');
      dispatch('saved');
      open = false;
      dispatch('close');
    } catch (e) {
      showError(`Could not apply current calibration: ${e}`);
    }
  }

  async function discardDraft() {
    busy = true;
    try {
      await restoreInitialSession();
      draftSource = 'session';
      hasDraftChanges = false;
      keepSessionChanges = false;
      step = 'intro';
      showStatus('Restored previous session calibration');
    } finally {
      busy = false;
    }
  }

  async function toggleMirrorDirection(index: number) {
    busy = true;
    try {
      const nextSign: 1 | -1 = mirrorSigns[index] === -1 ? 1 : -1;
      mirrorSigns = await setMirrorSign(index, nextSign);
      const current = await getCalibrationState();
      applyDraft(current);
      hasDraftChanges = true;
      showStatus(`${JOINT_NAMES[index]} direction ${nextSign === 1 ? 'normal' : 'inverted'}`);
    } catch (e) {
      showError(`Direction update failed: ${e}`);
    } finally {
      busy = false;
    }
  }

  $: if (open && !wasOpen) {
    wasOpen = true;
    void initializeModal();
  } else if (!open && wasOpen) {
    wasOpen = false;
  }
</script>

{#if open}
  <div class="overlay" on:click|self={close} role="dialog">
    <div class="modal">
      <header class="modal-header">
        <div>
          <h3>Calibration</h3>
          <p class="subhead">Saved defaults persist across app restarts. Draft changes do not.</p>
        </div>
        <button class="close-btn" on:click={close} aria-label="Close">×</button>
      </header>

      <div class="modal-body">
        {#if step === 'intro'}
          <section class="intro">
            <div class="status-card">
              <div><strong>Saved calibration:</strong> {savedExists ? 'Available' : 'Not found yet'}</div>
              <div><strong>Current session:</strong> Ready to inspect or replace</div>
            </div>

            <p>
              Use a new capture when you want to recalibrate from the arms' current home pose. Use the
              saved calibration when you want to restore the last known good setup. Current-session edits
              stay temporary until you explicitly save them.
            </p>

            <div class="action-stack">
              <button class="btn primary" on:click={captureNewCalibration} disabled={busy}>
                Capture New Calibration
              </button>
              <button class="btn" on:click={reviewCurrentSession} disabled={busy}>
                Use Current Session Calibration
              </button>
              <button class="btn" on:click={loadSavedIntoDraft} disabled={busy || !savedExists}>
                Load Saved Calibration
              </button>
            </div>
          </section>
        {:else if step === 'capturing'}
          <div class="capturing">
            <p>Reading joint positions from both arms...</p>
          </div>
        {:else}
          <section class="review">
            <div class="status-card">
              <div><strong>Draft source:</strong> {calibrationLabel(draftSource)}</div>
              <div><strong>Saved calibration:</strong> {savedExists ? 'Available' : 'Not found yet'}</div>
            </div>

            <p>
              Review this draft before committing it. <em>Apply For This Session</em> keeps it until the
              app closes. <em>Save As Default</em> makes it the calibration that auto-loads next time.
              <em>Discard Draft</em> restores the session snapshot from when you opened this modal.
            </p>

            <table class="offsets-table">
              <thead>
                <tr>
                  <th>Joint</th>
                  <th>Leader</th>
                  <th>Follower</th>
                  <th>Offset</th>
                  <th>Direction</th>
                </tr>
              </thead>
              <tbody>
                {#each JOINT_NAMES as name, i}
                  <tr>
                    <td>{name}</td>
                    <td>{leaderPositions[i] ?? '-'}</td>
                    <td>{followerPositions[i] ?? '-'}</td>
                    <td class:positive={offsets[i] > 0} class:negative={offsets[i] < 0}>
                      {offsets[i] > 0 ? '+' : ''}{offsets[i] ?? 0}
                    </td>
                    <td>
                      <button class="dir-btn" on:click={() => toggleMirrorDirection(i)} disabled={busy}>
                        {mirrorSigns[i] === -1 ? 'Inverted' : 'Normal'}
                      </button>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>

            <div class="action-stack">
              <button class="btn primary" on:click={applyForSession} disabled={busy}>
                Apply For This Session
              </button>
              <button class="btn primary" on:click={saveAsDefault} disabled={busy}>
                Save As Default
              </button>
              <button class="btn" on:click={discardDraft} disabled={busy}>
                Discard Draft
              </button>
              <button class="btn" on:click={loadSavedIntoDraft} disabled={busy || !savedExists}>
                Reload Saved
              </button>
              <button class="btn" on:click={captureNewCalibration} disabled={busy}>
                Re-capture
              </button>
            </div>

            <p class="hint">
              Toggle a direction only when a joint moves the wrong way. Use re-capture when the direction is
              correct but the home pose is offset.
            </p>
          </section>
        {/if}
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
    max-width: 92vw;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
    overflow: hidden;
  }

  .modal-header {
    display: flex;
    align-items: flex-start;
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

  .subhead {
    margin: 6px 0 0;
    color: #7f93a8;
    font-size: 12px;
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
    padding: 18px;
    overflow-y: auto;
  }

  p {
    color: #aabbcc;
    font-size: 13px;
    line-height: 1.6;
    margin: 0 0 14px;
  }

  .status-card {
    display: grid;
    gap: 6px;
    margin-bottom: 14px;
    padding: 12px 14px;
    border: 1px solid #1e2a3a;
    border-radius: 6px;
    background: #111927;
    color: #c0d0e0;
    font-size: 12px;
  }

  .action-stack {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 12px;
  }

  .btn {
    padding: 8px 16px;
    border: 1px solid #2a3444;
    border-radius: 5px;
    cursor: pointer;
    font-size: 13px;
    background: #0f1520;
    color: #aabbcc;
    transition: all 0.12s;
  }

  .btn:hover:not(:disabled) {
    background: #1a2332;
    color: #c0d0e0;
  }

  .btn.primary {
    background: #1e3a5f;
    color: #60a5fa;
    border-color: #2563eb;
  }

  .btn.primary:hover:not(:disabled) {
    background: #1e4080;
  }

  .btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .dir-btn {
    padding: 4px 8px;
    border: 1px solid #2a3444;
    border-radius: 4px;
    background: #16202d;
    color: #c0d0e0;
    font-size: 11px;
    cursor: pointer;
  }

  .dir-btn:hover:not(:disabled) {
    background: #1c2c3f;
  }

  .dir-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .capturing {
    text-align: center;
    padding: 20px;
  }

  .offsets-table {
    width: 100%;
    border-collapse: collapse;
    margin: 8px 0;
    font-size: 12px;
    font-family: monospace;
  }

  .offsets-table th {
    text-align: left;
    color: #8899aa;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    padding: 6px 10px;
    border-bottom: 1px solid #1e2a3a;
  }

  .offsets-table td {
    padding: 6px 10px;
    color: #c0d0e0;
    border-bottom: 1px solid #14202e;
  }

  .offsets-table td.positive {
    color: #4ade80;
  }

  .offsets-table td.negative {
    color: #f87171;
  }

  em {
    color: #60a5fa;
    font-style: normal;
    font-family: monospace;
  }

  .hint {
    margin-top: 12px;
    color: #7f93a8;
    font-size: 12px;
  }
</style>
