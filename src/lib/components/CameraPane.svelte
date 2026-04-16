<script lang="ts">
  import { onMount } from 'svelte';
  import {
    findColor,
    hexToRgb,
    rgbToHex,
    rgbToHsv,
    type Hsv,
    type TrackResult
  } from '$lib/utils/color-tracking';

  let video: HTMLVideoElement;
  let processCanvas: HTMLCanvasElement;
  let overlayCanvas: HTMLCanvasElement;
  let devices: MediaDeviceInfo[] = [];
  let selectedDeviceId: string | null = null;
  let stream: MediaStream | null = null;
  let captureTimer: ReturnType<typeof setInterval> | null = null;
  let errorMessage = '';
  let status = 'Idle';
  let capturing = false;
  let lastFrameAt = '';
  const captureWidth = 640;
  const captureHeight = 480;

  let scanning = false;

  // Color tracking state
  let trackingEnabled = false;
  let targetColor = '#ff3030';
  let pickMode = false;
  let hueTolerance = 18;
  let satMin = 0.3;
  let valMin = 0.25;
  let minPixels = 40;
  let sampleStep = 2;
  let tracked: TrackResult | null = null;

  $: targetHsv = computeTargetHsv(targetColor);

  function computeTargetHsv(hex: string): Hsv {
    const { r, g, b } = hexToRgb(hex);
    return rgbToHsv(r, g, b);
  }

  async function getDevices() {
    const allDevices = await navigator.mediaDevices.enumerateDevices();
    console.log('enumerateDevices() result:', allDevices);
    devices = allDevices.filter((d) => d.kind === 'videoinput');

    if (devices.length > 0 && selectedDeviceId && !devices.some((device) => device.deviceId === selectedDeviceId)) {
      selectedDeviceId = null;
    }
  }

  async function rescanCameras() {
    scanning = true;
    errorMessage = '';
    const prevCount = devices.length;
    try {
      if (!stream) {
        try {
          const tempStream = await navigator.mediaDevices.getUserMedia({ video: true, audio: false });
          tempStream.getTracks().forEach((t) => t.stop());
        } catch {
          // Permission denied or no camera — enumerate anyway, labels may be blank
        }
      }
      await getDevices();
      const newCount = devices.length;
      status = `Found ${newCount} camera${newCount === 1 ? '' : 's'}${
        newCount > prevCount ? ` (+${newCount - prevCount} new)` : ''
      }`;
    } catch (error) {
      errorMessage = `Scan failed: ${stringifyError(error)}`;
    } finally {
      scanning = false;
    }
  }

  function stopStream() {
    if (stream) {
      stream.getTracks().forEach((track) => track.stop());
      stream = null;
    }
  }

  function stopCapture() {
    if (captureTimer) {
      clearInterval(captureTimer);
      captureTimer = null;
    }
    capturing = false;
  }

  function captureFrame() {
    if (!processCanvas || !video || video.readyState < 2) return;
    const ctx = processCanvas.getContext('2d');
    if (!ctx) return;

    ctx.drawImage(video, 0, 0, processCanvas.width, processCanvas.height);
    lastFrameAt = new Date().toLocaleTimeString();

    if (trackingEnabled) {
      const img = ctx.getImageData(0, 0, processCanvas.width, processCanvas.height);
      tracked = findColor(img.data, processCanvas.width, processCanvas.height, targetHsv, {
        hueTolerance,
        satMin,
        valMin,
        minPixels,
        step: sampleStep
      });
    } else {
      tracked = null;
    }
    drawOverlay();
  }

  function drawOverlay() {
    if (!overlayCanvas) return;
    const ctx = overlayCanvas.getContext('2d');
    if (!ctx) return;
    ctx.clearRect(0, 0, overlayCanvas.width, overlayCanvas.height);
    if (!tracked) return;

    ctx.strokeStyle = '#39ff7a';
    ctx.lineWidth = 3;
    ctx.strokeRect(tracked.x, tracked.y, Math.max(tracked.w, 1), Math.max(tracked.h, 1));

    ctx.beginPath();
    ctx.moveTo(tracked.cx - 12, tracked.cy);
    ctx.lineTo(tracked.cx + 12, tracked.cy);
    ctx.moveTo(tracked.cx, tracked.cy - 12);
    ctx.lineTo(tracked.cx, tracked.cy + 12);
    ctx.stroke();
  }

  function startCapture() {
    stopCapture();
    captureFrame();
    captureTimer = setInterval(captureFrame, 100);
    capturing = true;
  }

  function stringifyError(error: unknown) {
    if (error instanceof Error) {
      return `${error.name}: ${error.message}`;
    }

    return String(error);
  }

  async function requestCameraStream(
    constraints: MediaStreamConstraints,
    label: string
  ): Promise<MediaStream> {
    console.log(`${label} getUserMedia constraints:`, constraints);
    status = label;
    return navigator.mediaDevices.getUserMedia(constraints);
  }

  async function attachStream(nextStream: MediaStream, activeStatus: string) {
    stream = nextStream;
    video.srcObject = stream;
    await video.play();
    status = activeStatus;
    errorMessage = '';
    startCapture();
  }

  async function startDefaultCamera() {
    errorMessage = '';
    lastFrameAt = '';
    stopCapture();
    stopStream();

    try {
      const nextStream = await requestCameraStream(
        {
          video: true,
          audio: false
        },
        'Auto-starting default camera'
      );
      await attachStream(nextStream, 'Live preview active');
    } catch (error) {
      const failureText = stringifyError(error);
      console.error('Default camera startup failed:', error);
      status = `Startup failed: ${failureText}`;
      errorMessage = `Startup failed: ${failureText}`;
    }
  }

  async function startSelectedCamera() {
    errorMessage = '';
    lastFrameAt = '';
    stopCapture();
    stopStream();

    if (!selectedDeviceId) {
      status = 'Live preview active';
      return;
    }

    const selectedConstraints: MediaStreamConstraints = {
      video: {
        deviceId: { exact: selectedDeviceId },
        width: captureWidth,
        height: captureHeight
      },
      audio: false
    };

    try {
      const nextStream = await requestCameraStream(
        selectedConstraints,
        'Switching to selected camera...'
      );
      await attachStream(nextStream, 'Switched to selected camera');
    } catch (error) {
      const failureText = stringifyError(error);
      console.error('Selected camera startup failed:', error);
      status = `Camera switch failed: ${failureText}`;
      errorMessage = `Camera switch failed: ${failureText}`;
    }
  }

  function handleVideoClick(event: MouseEvent) {
    if (!pickMode || !processCanvas || !video) return;
    const ctx = processCanvas.getContext('2d');
    if (!ctx) return;

    const rect = video.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return;
    const sx = Math.floor(((event.clientX - rect.left) / rect.width) * processCanvas.width);
    const sy = Math.floor(((event.clientY - rect.top) / rect.height) * processCanvas.height);

    ctx.drawImage(video, 0, 0, processCanvas.width, processCanvas.height);
    const px = ctx.getImageData(
      Math.max(0, Math.min(processCanvas.width - 1, sx)),
      Math.max(0, Math.min(processCanvas.height - 1, sy)),
      1,
      1
    ).data;
    targetColor = rgbToHex(px[0], px[1], px[2]);
    pickMode = false;
    if (!trackingEnabled) trackingEnabled = true;
  }

  onMount(() => {
    void (async () => {
      try {
        status = 'Auto-starting default camera';
        await startDefaultCamera();
        await getDevices();
        if (devices.length > 0 && !selectedDeviceId) {
          selectedDeviceId = devices[0].deviceId;
        }
      } catch (error) {
        const message = stringifyError(error);
        errorMessage = `Startup failed: ${message}`;
        status = `Startup failed: ${message}`;
      }
    })();

    return () => {
      stopCapture();
      stopStream();
    };
  });
</script>

<div class="camera-pane">
  <h3>Camera</h3>

  <div class="controls">
    <select bind:value={selectedDeviceId} on:change={startSelectedCamera} disabled={devices.length === 0}>
      {#if devices.length === 0}
        <option value={null}>No cameras found</option>
      {/if}
      {#each devices as d}
        <option value={d.deviceId}>{d.label || `Camera ${d.deviceId.slice(0, 6)}`}</option>
      {/each}
    </select>
    <button class="scan-btn" on:click={rescanCameras} disabled={scanning}>
      {scanning ? 'Scanning...' : 'Rescan cameras'}
    </button>
  </div>

  <div class="status-row">
    <span>{status}</span>
    {#if capturing}
      <span>Capturing frames at ~10 FPS</span>
    {/if}
  </div>

  {#if errorMessage}
    <div class="error">{errorMessage}</div>
  {/if}

  <div class="video-wrap" class:picking={pickMode}>
    <video
      bind:this={video}
      autoplay
      playsinline
      on:click={handleVideoClick}
    ></video>
    <canvas
      bind:this={overlayCanvas}
      class="overlay"
      width={captureWidth}
      height={captureHeight}
      on:click={handleVideoClick}
    ></canvas>
  </div>

  <canvas
    bind:this={processCanvas}
    class="hidden-canvas"
    width={captureWidth}
    height={captureHeight}
  ></canvas>

  <div class="track-controls">
    <label class="toggle">
      <input type="checkbox" bind:checked={trackingEnabled} />
      Track color
    </label>

    <label class="color-picker" title="Pick a target color">
      <input type="color" bind:value={targetColor} />
      <span class="swatch" style="background: {targetColor}"></span>
      <span class="hex">{targetColor.toUpperCase()}</span>
    </label>

    <button
      class="scan-btn"
      class:active={pickMode}
      on:click={() => (pickMode = !pickMode)}
      disabled={!capturing}
    >
      {pickMode ? 'Click on video...' : 'Pick from camera'}
    </button>

    <label class="slider">
      <span>Hue ±{hueTolerance}°</span>
      <input type="range" min="2" max="60" step="1" bind:value={hueTolerance} />
    </label>

    <label class="slider">
      <span>Sat ≥ {satMin.toFixed(2)}</span>
      <input type="range" min="0" max="1" step="0.05" bind:value={satMin} />
    </label>

    <label class="slider">
      <span>Val ≥ {valMin.toFixed(2)}</span>
      <input type="range" min="0" max="1" step="0.05" bind:value={valMin} />
    </label>
  </div>

  {#if trackingEnabled}
    <div class="track-info">
      {#if tracked}
        <span>centroid: ({Math.round(tracked.cx)}, {Math.round(tracked.cy)})</span>
        <span>box: {tracked.w}×{tracked.h}</span>
        <span>pixels: {tracked.count}</span>
      {:else}
        <span class="dim">No match (try a wider hue or lower sat/val)</span>
      {/if}
    </div>
  {/if}

  {#if lastFrameAt}
    <div class="frame-meta">Last frame captured: {lastFrameAt}</div>
  {/if}
</div>

<style>
  .camera-pane {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border: 1px solid #1e2a3a;
    border-radius: 6px;
    background: #0a0e17;
  }

  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: #d8dee9;
  }

  .controls {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .status-row {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: 12px;
    color: #8aa0b8;
  }

  .error {
    padding: 8px 10px;
    border: 1px solid #5b1f29;
    border-radius: 4px;
    background: #2a1117;
    color: #ff9aa5;
    font-size: 12px;
  }

  select {
    padding: 6px 10px;
    background: #111827;
    color: #e0e0e0;
    border: 1px solid #2a3444;
    border-radius: 4px;
    font-size: 12px;
  }

  select:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .scan-btn {
    padding: 6px 12px;
    background: #111827;
    color: #8899aa;
    border: 1px solid #2a3444;
    border-radius: 4px;
    font-size: 12px;
    cursor: pointer;
    transition: all 0.12s;
    white-space: nowrap;
  }

  .scan-btn:hover:not(:disabled) {
    background: #1a2332;
    color: #c0d0e0;
    border-color: #3a4a5a;
  }

  .scan-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .scan-btn.active {
    background: #1e3a5f;
    border-color: #2563eb;
    color: #60a5fa;
  }

  .video-wrap {
    position: relative;
    display: inline-block;
    max-width: 720px;
    width: 100%;
  }

  .video-wrap.picking video,
  .video-wrap.picking .overlay {
    cursor: crosshair;
  }

  video {
    width: 100%;
    display: block;
    border: 2px solid #444;
    border-radius: 4px;
    background: #05070b;
  }

  .overlay {
    position: absolute;
    inset: 2px;
    width: calc(100% - 4px);
    height: calc(100% - 4px);
    pointer-events: none;
    border-radius: 2px;
  }

  .video-wrap.picking .overlay {
    pointer-events: auto;
  }

  .hidden-canvas {
    display: none;
  }

  .track-controls {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 14px;
    padding: 8px 10px;
    background: #0d1320;
    border: 1px solid #1e2a3a;
    border-radius: 4px;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: #c0d0e0;
    cursor: pointer;
  }

  .color-picker {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: #8aa0b8;
    cursor: pointer;
  }

  .color-picker input[type='color'] {
    width: 28px;
    height: 22px;
    border: 1px solid #2a3444;
    border-radius: 3px;
    background: transparent;
    padding: 0;
    cursor: pointer;
  }

  .swatch {
    display: none;
  }

  .hex {
    font-family: monospace;
    font-size: 11px;
    color: #c0d0e0;
  }

  .slider {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: #8aa0b8;
  }

  .slider span {
    min-width: 78px;
    font-family: monospace;
  }

  .slider input[type='range'] {
    width: 110px;
  }

  .track-info {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    padding: 6px 10px;
    background: #08111c;
    border: 1px solid #1e2a3a;
    border-radius: 4px;
    font-family: monospace;
    font-size: 11px;
    color: #4ade80;
  }

  .track-info .dim {
    color: #6f8193;
  }

  .frame-meta {
    font-size: 11px;
    color: #6f8193;
  }
</style>
