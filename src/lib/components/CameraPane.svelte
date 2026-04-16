<script lang="ts">
  import { onMount } from 'svelte';

  let video: HTMLVideoElement;
  let canvas: HTMLCanvasElement;
  let devices: MediaDeviceInfo[] = [];
  let selectedDeviceId: string | null = null;
  let stream: MediaStream | null = null;
  let captureTimer: ReturnType<typeof setInterval> | null = null;
  let errorMessage = '';
  let status = 'Idle';
  let capturing = false;
  let lastFrameAt = '';
  let showCanvas = false;
  const captureWidth = 640;
  const captureHeight = 480;

  async function getDevices() {
    const allDevices = await navigator.mediaDevices.enumerateDevices();
    console.log('enumerateDevices() result:', allDevices);
    devices = allDevices.filter((d) => d.kind === 'videoinput');

    if (devices.length > 0 && selectedDeviceId && !devices.some((device) => device.deviceId === selectedDeviceId)) {
      selectedDeviceId = null;
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
    if (!canvas || !video || video.readyState < 2) {
      return;
    }

    const ctx = canvas.getContext('2d');
    if (!ctx) {
      return;
    }

    ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
    lastFrameAt = new Date().toLocaleTimeString();
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
      {#each devices as d}
        <option value={d.deviceId}>{d.label || 'Camera'}</option>
      {/each}
    </select>
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

  <video bind:this={video} autoplay playsinline></video>
  {#if showCanvas}
    <canvas bind:this={canvas} width={captureWidth} height={captureHeight}></canvas>
  {:else}
    <canvas bind:this={canvas} width={captureWidth} height={captureHeight} class="hidden-canvas"></canvas>
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

  video {
    width: 100%;
    max-width: 720px;
    border: 2px solid #444;
    border-radius: 4px;
    background: #05070b;
  }

  canvas {
    width: 100%;
    max-width: 640px;
    border: 1px solid #2a3444;
    border-radius: 4px;
    background: #05070b;
  }

  .hidden-canvas {
    display: none;
  }

  .frame-meta {
    font-size: 11px;
    color: #6f8193;
  }
</style>
