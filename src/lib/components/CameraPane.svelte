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
  import {
    emptyCalibration,
    fitAffines,
    fitQuadratics,
    applyAffines,
    applyQuadratics,
    insideHull,
    saveCalibrationToLocalStorage,
    loadCalibrationFromLocalStorage,
    type VisionCalibration,
  } from '$lib/utils/vision-calibration';
  import { followerJoints, isMirroring } from '$lib/stores/joints';
  import { followerConnection } from '$lib/stores/connection';
  import { writeAllJoints, setTorque } from '$lib/tauri/commands';

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

  // Free-pick color tracking state (existing feature)
  let trackingEnabled = false;
  let targetColor = '#ff3030';
  let pickMode = false;
  let hueTolerance = 18;
  let satMin = 0.3;
  let valMin = 0.25;
  let minPixels = 40;
  let sampleStep = 2;
  let tracked: TrackResult | null = null;

  // Block tracking (drives follower) — separate color slot from above
  let blockColor = '#ffd000';
  let blockPickMode = false;
  let blockHueTol = 18;
  let blockSatMin = 0.35;
  let blockValMin = 0.75;
  let blockTracked: TrackResult | null = null;

  // Vision calibration: 4-point pixel→joint affine.
  // Two passes: hover-height samples (current `samples`) and pickup-height
  // samples taken at the same XY but with the gripper tip on the table.
  let calibration: VisionCalibration = emptyCalibration(captureWidth, captureHeight);
  let calibrationMode = false;
  let calibrationPass: 'hover' | 'pickup' = 'hover';
  let calibrationClickArm = false; // when true, next click captures a sample
  let dropSpotClickArm = false;     // when true, next click sets the drop pixel

  // Follower tracking
  let followerTrackingEnabled = false;
  /** Max raw counts the commanded joints can step per send. ~30 ≈ 2.6° per call. */
  const MAX_JOINT_STEP = 30;
  let lastSentTargets: number[] | null = null;
  let trackingStatus = '';

  // Gripper state machine.
  //   'auto'   — affine drives joint 5 (default)
  //   'closed' — overridden to gripperClosedRaw (auto-grabbed)
  //   'open'   — overridden to gripperOpenRaw (release button held)
  type GripperMode = 'auto' | 'closed' | 'open';
  let gripperMode: GripperMode = 'auto';
  let prevGripperMode: GripperMode = 'auto';
  let autoGrabArmed = false;
  let gripperClosedRaw = 3200;
  let gripperOpenRaw = 4095;
  /** Raw counts/tick for the gripper joint. Faster than the arm but not
   *  instant — instant slams cause motor overload when the jaw hits a block. */
  let gripperStep = 40;
  /** Pixel bias applied to the frozen pickup centroid (hover + all PnP legs).
   *  Use to compensate for centroid bias / lens distortion. */
  let pickBiasX = 0;
  let pickBiasY = 0;
  /** Descent-only pixel bias for pickup: applied to the target pixel during
   *  descend_pick / close_pick (not during hover or lift). Use this when the
   *  gripper drifts laterally between hover and table due to the arm's arc;
   *  set the bias OPPOSITE the drift to compensate. */
  let pickDescBiasX = 0;
  let pickDescBiasY = 0;
  /** Same idea for the drop leg. */
  let dropDescBiasX = 0;
  let dropDescBiasY = 0;

  // Stability detection for auto-grab — radius is adjustable since color
  // tracking can jitter even when the block is physically still.
  let stabilityPixelRadius = 30;
  const STABILITY_WINDOW_MS = 600;
  const ARRIVAL_TOL = 60; // raw counts, joints 0..4
  let centroidHistory: { x: number; y: number; t: number }[] = [];
  let stableSinceMs: number | null = null;
  let stabilityProgress = 0; // 0..1

  // Pick-and-place state machine.
  // Sequence: idle (free tracking) → user runs → await stable over block →
  //   descend to pickup pose → close gripper → lift back to hover →
  //   move to drop pixel → await stable → descend → open gripper →
  //   lift → idle.
  type PnPState =
    | 'idle'
    | 'await_block'
    | 'descend_pick'
    | 'close_pick'
    | 'lift_pick'
    | 'move_to_drop'
    | 'await_drop'
    | 'descend_drop'
    | 'open_drop'
    | 'lift_drop';
  let pnpState: PnPState = 'idle';
  let pnpStateEnteredMs = 0;
  let pnpFrozenPx: { x: number; y: number } | null = null;
  /** Visual-servoed target pixel during descend_pick — low-pass filtered
   *  toward the live block centroid when visible, held when occluded. Locked
   *  back into pnpFrozenPx when descent transitions to close_pick. */
  let smoothedPickPx: { x: number; y: number } | null = null;
  /** Same idea for the drop side: while the gripper carries the block to
   *  the drop spot, the block centroid tells us where the gripper actually
   *  ended up; we feed that error back to nudge the commanded target. */
  let smoothedDropPx: { x: number; y: number } | null = null;
  /** Step-rate boosts for elbow_flex (joint 2) and wrist_flex (joint 3)
   *  during PnP descent. Higher = these joints finish their motion before
   *  shoulder catches up, producing a "fold-in then settle" path instead
   *  of "out and around then down". Endpoint is unchanged; only the path
   *  through joint space is shaped. */
  let elbowFirstBoost = 1.0;
  let wristFirstBoost = 1.0;
  /** Raw-count adjustments added to the descent ENDPOINT for the height
   *  joints. Use these to "coil" the arm — e.g. add elbow flex and reduce
   *  shoulder lift to make the arm curl in tightly during descent. The
   *  bias X/Y sliders can compensate for any XY drift these introduce. */
  let liftExtra = -250;  // signed counts added to shoulder_lift target
  let elbowExtra = 310;  // signed counts added to elbow_flex target
  let wristExtra = 210;  // signed counts added to wrist_flex target
  /** When on, use live centroid corrections during the descent. Off keeps
   *  the original one-shot behavior (target frozen at stability fire). */
  let liveCorrectDescent = true;
  /** Smoothing factor for the descent-time correction. Higher = more
   *  responsive (and more jittery); lower = more sluggish (and stable). */
  let liveCorrectAlpha = 0.75;
  /** Max pixel radius the live correction can drift from the original
   *  frozen target. Prevents occlusion-biased centroids from yanking the
   *  arm to a totally wrong spot. */
  let liveCorrectRadius = 170;
  const PNP_GRIP_HOLD_MS = 700;
  const PNP_RELEASE_HOLD_MS = 500;
  const PNP_MOVE_TIMEOUT_MS = 6000;   // safety cap on each motion leg
  const PNP_MIN_DWELL_MS = 200;       // small safety so command actually goes out
  let pnpStep = 35;                   // raw counts/tick during descend/lift/move
  const PNP_ARRIVAL_TOL = 60;         // raw counts, planar joints
  const BLOCK_HOLD_MS = 1500;         // hold last centroid through occlusion
  /** Per-leg descent depth multipliers. 1.0 lands at calibrated pickup pose,
   *  < 1.0 stops short, > 1.0 extrapolates beyond. Pickup and drop are
   *  separate because the same calibration can overshoot at one and land
   *  perfectly at the other (geometry varies across the workspace). */
  let pickDepth = 0.80;
  let dropDepth = 0.80;
  /** Currently-active depth, set by the state machine before descendTarget is called. */
  let descentDepth = 1.0;
  /** Force the captured global descent Δ instead of the per-pixel pickup fit.
   *  Useful when the pickup samples didn't capture meaningful descent (e.g.
   *  user clicked twice without lowering the gripper between clicks). */
  let forceGlobalDescent = false;

  // Two-stage descent vector capture. First click records the hover joints,
  // second click (after the user manually lowers the gripper) records the
  // ground joints; the difference is saved as a global descent delta that
  // replaces the pickup-affine descent.
  let descentCaptureStage: 'idle' | 'awaiting_low' = 'idle';
  let descentHoverRef: number[] | null = null;

  $: targetHsv = computeTargetHsv(targetColor);
  $: blockHsv = computeTargetHsv(blockColor);
  $: calibrationReady = calibration.affines !== null && calibration.samples.length >= 3;
  $: pickupReady =
    calibration.pickupAffines !== null && calibration.pickupSamples.length >= 3;
  $: descentReady = pickupReady || (calibration.descentDelta !== null);
  $: pickAndPlaceReady = calibrationReady && descentReady && calibration.dropSpot !== null;

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

    const needBlock = followerTrackingEnabled || calibrationMode;
    if (trackingEnabled || needBlock) {
      const img = ctx.getImageData(0, 0, processCanvas.width, processCanvas.height);
      if (trackingEnabled) {
        tracked = findColor(img.data, processCanvas.width, processCanvas.height, targetHsv, {
          hueTolerance,
          satMin,
          valMin,
          minPixels,
          step: sampleStep,
        });
      } else {
        tracked = null;
      }
      if (needBlock) {
        blockTracked = findColor(img.data, processCanvas.width, processCanvas.height, blockHsv, {
          hueTolerance: blockHueTol,
          satMin: blockSatMin,
          valMin: blockValMin,
          minPixels,
          step: sampleStep,
        });
      } else {
        blockTracked = null;
      }
    } else {
      tracked = null;
      blockTracked = null;
    }
    drawOverlay();

    if (followerTrackingEnabled) {
      void driveFollower();
    }
  }

  function drawOverlay() {
    if (!overlayCanvas) return;
    const ctx = overlayCanvas.getContext('2d');
    if (!ctx) return;
    ctx.clearRect(0, 0, overlayCanvas.width, overlayCanvas.height);

    if (tracked) {
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

    if (blockTracked) {
      ctx.strokeStyle = '#ffb000';
      ctx.lineWidth = 2;
      ctx.setLineDash([6, 4]);
      ctx.strokeRect(blockTracked.x, blockTracked.y, Math.max(blockTracked.w, 1), Math.max(blockTracked.h, 1));
      ctx.setLineDash([]);
      ctx.fillStyle = '#ffb000';
      ctx.beginPath();
      ctx.arc(blockTracked.cx, blockTracked.cy, 4, 0, Math.PI * 2);
      ctx.fill();
    }

    // Hover calibration markers (blue)
    if (calibration.samples.length > 0) {
      ctx.fillStyle = '#60a5fa';
      ctx.strokeStyle = '#1e40af';
      ctx.lineWidth = 2;
      ctx.font = '12px monospace';
      calibration.samples.forEach((s, i) => {
        ctx.beginPath();
        ctx.arc(s.px, s.py, 6, 0, Math.PI * 2);
        ctx.fill();
        ctx.stroke();
        ctx.fillStyle = '#ffffff';
        ctx.fillText(String(i + 1), s.px + 8, s.py + 4);
        ctx.fillStyle = '#60a5fa';
      });
    }
    // Pickup calibration markers (cyan, smaller, ringed)
    if (calibration.pickupSamples.length > 0) {
      ctx.strokeStyle = '#22d3ee';
      ctx.lineWidth = 2;
      ctx.fillStyle = '#0ea5b7';
      calibration.pickupSamples.forEach((s) => {
        ctx.beginPath();
        ctx.arc(s.px, s.py, 4, 0, Math.PI * 2);
        ctx.stroke();
        ctx.beginPath();
        ctx.arc(s.px, s.py, 2, 0, Math.PI * 2);
        ctx.fill();
      });
    }
    // Drop spot crosshair
    if (calibration.dropSpot) {
      const { px, py } = calibration.dropSpot;
      ctx.strokeStyle = '#f472b6';
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(px - 14, py);
      ctx.lineTo(px + 14, py);
      ctx.moveTo(px, py - 14);
      ctx.lineTo(px, py + 14);
      ctx.stroke();
      ctx.beginPath();
      ctx.arc(px, py, 10, 0, Math.PI * 2);
      ctx.stroke();
    }
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
    if (!processCanvas || !video) return;
    const ctx = processCanvas.getContext('2d');
    if (!ctx) return;

    const rect = video.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return;
    const sx = Math.floor(((event.clientX - rect.left) / rect.width) * processCanvas.width);
    const sy = Math.floor(((event.clientY - rect.top) / rect.height) * processCanvas.height);
    const cx = Math.max(0, Math.min(processCanvas.width - 1, sx));
    const cy = Math.max(0, Math.min(processCanvas.height - 1, sy));

    if (calibrationClickArm) {
      const joints = $followerJoints.positions.slice();
      if (joints.length !== 6) {
        errorMessage = 'Follower joint state not available — connect the follower first.';
        return;
      }
      const sample = { px: cx, py: cy, joints };
      if (calibrationPass === 'pickup') {
        calibration.pickupSamples = [...calibration.pickupSamples, sample];
        calibration.pickupAffines = fitAffines(calibration.pickupSamples);
        calibration.pickupQuadratics = fitQuadratics(calibration.pickupSamples);
      } else {
        calibration.samples = [...calibration.samples, sample];
        calibration.affines = fitAffines(calibration.samples);
        calibration.quadratics = fitQuadratics(calibration.samples);
      }
      saveCalibrationToLocalStorage(calibration);
      calibrationClickArm = false;
      drawOverlay();
      return;
    }

    if (dropSpotClickArm) {
      calibration.dropSpot = { px: cx, py: cy };
      saveCalibrationToLocalStorage(calibration);
      dropSpotClickArm = false;
      drawOverlay();
      return;
    }

    if (pickMode || blockPickMode) {
      ctx.drawImage(video, 0, 0, processCanvas.width, processCanvas.height);
      const px = ctx.getImageData(cx, cy, 1, 1).data;
      const hex = rgbToHex(px[0], px[1], px[2]);
      if (blockPickMode) {
        blockColor = hex;
        blockPickMode = false;
      } else {
        targetColor = hex;
        pickMode = false;
        if (!trackingEnabled) trackingEnabled = true;
      }
    }
  }

  function startCalibration() {
    calibrationMode = true;
    calibrationPass = 'hover';
    // Preserve descent Δ and drop spot across re-calibrations — they're
    // independent of the hover/pickup samples and tedious to redo.
    const preservedDelta = calibration.descentDelta;
    const preservedDrop = calibration.dropSpot;
    calibration = emptyCalibration(captureWidth, captureHeight);
    calibration.descentDelta = preservedDelta;
    calibration.dropSpot = preservedDrop;
    saveCalibrationToLocalStorage(calibration);
  }

  function finishCalibration() {
    calibration.affines = fitAffines(calibration.samples);
    calibration.quadratics = fitQuadratics(calibration.samples);
    calibration.pickupAffines = fitAffines(calibration.pickupSamples);
    calibration.pickupQuadratics = fitQuadratics(calibration.pickupSamples);
    saveCalibrationToLocalStorage(calibration);
    calibrationMode = false;
    calibrationClickArm = false;
  }

  function clearCalibration() {
    calibration = emptyCalibration(captureWidth, captureHeight);
    saveCalibrationToLocalStorage(calibration);
    drawOverlay();
  }

  function removeLastSample() {
    if (calibrationPass === 'pickup') {
      if (calibration.pickupSamples.length === 0) return;
      calibration.pickupSamples = calibration.pickupSamples.slice(0, -1);
      calibration.pickupAffines = fitAffines(calibration.pickupSamples);
      calibration.pickupQuadratics = fitQuadratics(calibration.pickupSamples);
    } else {
      if (calibration.samples.length === 0) return;
      calibration.samples = calibration.samples.slice(0, -1);
      calibration.affines = fitAffines(calibration.samples);
      calibration.quadratics = fitQuadratics(calibration.samples);
    }
    saveCalibrationToLocalStorage(calibration);
    drawOverlay();
  }

  function setDropSpot() {
    dropSpotClickArm = !dropSpotClickArm;
    calibrationClickArm = false;
    pickMode = false;
    blockPickMode = false;
  }

  async function captureDescentStep() {
    if (!$followerConnection?.connected) {
      errorMessage = 'Connect the follower first.';
      return;
    }
    const joints = $followerJoints.positions.slice();
    if (joints.length !== 6) {
      errorMessage = 'Follower joint state not available.';
      return;
    }

    if (descentCaptureStage === 'idle') {
      // First click: stop tracking and detorque so the user can hand-lower
      // the gripper without the controller fighting back.
      if (followerTrackingEnabled) {
        followerTrackingEnabled = false;
        lastSentTargets = null;
      }
      try {
        await setTorque('follower', false);
      } catch (e) {
        errorMessage = `setTorque failed: ${e}`;
      }
      descentHoverRef = joints;
      descentCaptureStage = 'awaiting_low';
      trackingStatus = 'descent capture: lower gripper to table, then click again';
      return;
    }

    // Second click: compute delta, save, restore torque so the arm holds.
    const delta = joints.map((v, i) => v - (descentHoverRef?.[i] ?? v));
    calibration.descentDelta = delta;
    saveCalibrationToLocalStorage(calibration);
    descentHoverRef = null;
    descentCaptureStage = 'idle';
    try {
      await setTorque('follower', true);
    } catch (e) {
      errorMessage = `setTorque restore failed: ${e}`;
    }
    const maxAbs = Math.max(...delta.slice(0, 5).map((d) => Math.abs(d)));
    trackingStatus =
      maxAbs < 30
        ? `descent captured but tiny (max |Δ|=${maxAbs}) — did the arm actually move?`
        : `descent captured: max |Δ|=${maxAbs}`;
    errorMessage = '';
  }

  function clearDescentDelta() {
    calibration.descentDelta = null;
    saveCalibrationToLocalStorage(calibration);
    descentHoverRef = null;
    descentCaptureStage = 'idle';
  }

  function clampStep(target: number, current: number): number {
    return clampPerJointStep(target, current, MAX_JOINT_STEP);
  }

  function clampPerJointStep(target: number, current: number, maxStep: number): number {
    const d = target - current;
    if (d > maxStep) return current + maxStep;
    if (d < -maxStep) return current - maxStep;
    return target;
  }

  /**
   * Coordinated step: all joints arrive at target on the same tick by
   * scaling each joint's step proportionally. The longest-traveling joint
   * uses the full maxStep; shorter ones move proportionally less. Result
   * is a smoother end-effector path (closer to straight-line) than per-
   * joint independent stepping where short-delta joints finish early and
   * the others trace a curve.
   *
   * `excludeIdx` (e.g. gripper) keeps its own independent step.
   */
  function coordinatedStep(
    targets: number[],
    lastSent: number[],
    maxStep: number,
    excludeIdx: number,
    perJointBoost?: number[],
  ): number[] {
    let maxAbs = 0;
    const deltas: number[] = [];
    for (let i = 0; i < targets.length; i++) {
      if (i === excludeIdx) { deltas.push(0); continue; }
      const d = targets[i] - lastSent[i];
      // Effective travel distance for the proportional scale: a joint with
      // a per-joint boost > 1 is treated as "shorter" so it gets the same
      // step magnitude as the longest joint, finishing earlier.
      const effective = perJointBoost?.[i] ? Math.abs(d) / perJointBoost[i] : Math.abs(d);
      deltas.push(d);
      if (effective > maxAbs) maxAbs = effective;
    }
    if (maxAbs <= maxStep) return targets.slice();
    const scale = maxStep / maxAbs;
    return targets.map((t, i) => {
      if (i === excludeIdx) return t;
      const boost = perJointBoost?.[i] ?? 1;
      const stepped = lastSent[i] + deltas[i] * scale * boost;
      // Don't overshoot the target.
      const d = t - lastSent[i];
      if (d > 0) return Math.round(Math.min(stepped, t));
      if (d < 0) return Math.round(Math.max(stepped, t));
      return Math.round(stepped);
    });
  }

  let driveBusy = false;
  let lastTrackTime = 0;
  /** Last successfully detected block centroid; used to bridge brief
   *  occlusion (e.g. when the gripper covers the block from the camera). */
  let lastBlockCentroid: { x: number; y: number; t: number } | null = null;
  /** Largest recent detected block area (pixel count), with slow decay.
   *  Used to gauge whether the current detection is the full block or a
   *  partially-occluded sliver. Detections below confidenceFloor × this
   *  are treated as untrustworthy and don't update the target. */
  let recentMaxBlockArea = 0;
  const AREA_DECAY_PER_FRAME = 0.995;
  /** Detection-area fraction below which we ignore the live centroid and
   *  hold the last trusted one. Higher = stricter (more occlusion bridging). */
  let confidenceFloor = 0.20;
  /** Pixel offset applied to the hover target during tracking and await_block,
   *  so the gripper sits behind/beside the block instead of directly over it.
   *  Keeps the block visible to an overhead camera, dramatically improving
   *  centroid stability during approach. The descent itself ignores this
   *  offset and targets the true block centroid. */
  let hoverOffsetX = 0;
  let hoverOffsetY = -50;

  function arrivedAt(target: number[], current: number[], jointCount: number): boolean {
    for (let i = 0; i < jointCount; i++) {
      if (Math.abs((current[i] ?? 0) - (target[i] ?? 0)) > PNP_ARRIVAL_TOL) return false;
    }
    return true;
  }

  async function driveFollower(): Promise<void> {
    if (driveBusy) return;
    if (!followerTrackingEnabled) return;
    if ($isMirroring) {
      trackingStatus = 'paused (mirror is on)';
      return;
    }
    if (!$followerConnection?.connected) {
      trackingStatus = 'paused (follower not connected)';
      return;
    }
    if ($followerJoints.timestamp === 0) {
      // Refuse to command until ArmPane's polling has given us a real read.
      // Otherwise we'd start stepping from the default [2048,...] initial
      // store value and yank the arm to "all-center" pose.
      trackingStatus = 'waiting for first joint poll...';
      return;
    }
    if (!calibration.affines || calibration.samples.length < 3) {
      trackingStatus = 'need at least 3 calibration samples';
      return;
    }

    // Refresh last-known block centroid for occlusion bridging — but only
    // when the detection looks confident (area near the recent max). When
    // the gripper covers the block, the detected area drops dramatically
    // and the centroid is biased toward whatever sliver of color is left;
    // updating the target with that causes the arm to chase the sliver.
    if (blockTracked) {
      const area = blockTracked.count;
      if (area > recentMaxBlockArea) {
        recentMaxBlockArea = area;
      } else {
        recentMaxBlockArea = Math.max(recentMaxBlockArea * AREA_DECAY_PER_FRAME, area);
      }
      if (area >= recentMaxBlockArea * confidenceFloor) {
        lastBlockCentroid = { x: blockTracked.cx, y: blockTracked.cy, t: Date.now() };
      }
    }

    // Pick-and-place state machine first; idle falls through to free tracking.
    const command = computePnpCommand();
    if (!command) return;

    const current = $followerJoints.positions;
    const step = command.maxStep ?? MAX_JOINT_STEP;
    const lastSent = command.targets.map(
      (_, i) => lastSentTargets?.[i] ?? current[i] ?? 2048,
    );

    let stepped: number[];
    if (pnpState !== 'idle') {
      // Coordinated stepping for PnP — all arm joints arrive together so
      // the end-effector traces a much straighter path through space.
      // Elbow gets a step boost during descent legs so it folds in first
      // (gripper drops more vertically) before shoulder finishes its motion.
      const isDescent =
        pnpState === 'descend_pick' ||
        pnpState === 'close_pick' ||
        pnpState === 'descend_drop' ||
        pnpState === 'open_drop';
      const perJointBoost = isDescent
        ? [1, 1, elbowFirstBoost, wristFirstBoost, 1, 1]
        : undefined;
      stepped = coordinatedStep(command.targets, lastSent, step, 5, perJointBoost);
      // Gripper steps independently with its own (looser) cap.
      stepped[5] = clampPerJointStep(command.targets[5], lastSent[5], gripperStep);
    } else {
      // Free tracking — per-joint independent stepping is fine here
      // because the target updates continuously and we never have a
      // long discrete leg to traverse.
      stepped = command.targets.map((t, i) => {
        const cap = i === 5 ? gripperStep : step;
        return clampPerJointStep(t, lastSent[i], cap);
      });
    }

    // Only the idle state's autoGrabArmed flow runs stability here. The
    // await_block state runs its own updateStability() inside computePnpCommand
    // with active=true, so don't clobber its progress here.
    if (pnpState === 'idle') {
      if (blockTracked) {
        updateStability(blockTracked, current, command.affineTargets);
      } else {
        stableSinceMs = null;
        stabilityProgress = 0;
      }
    }

    driveBusy = true;
    try {
      await writeAllJoints('follower', stepped);
      lastSentTargets = stepped;
      trackingStatus = command.label;
    } catch (e) {
      trackingStatus = `write failed: ${e}`;
    } finally {
      driveBusy = false;
    }
  }

  /**
   * Returns the joint targets to send this tick based on the pick-and-place
   * state machine, or null if no command should be sent (e.g. idle with no
   * block detected). Also advances state on timer/condition.
   */
  function computePnpCommand():
    | { targets: number[]; affineTargets: number[]; label: string; maxStep?: number }
    | null {
    const now = Date.now();
    const elapsed = now - pnpStateEnteredMs;
    const drop = calibration.dropSpot;

    // Mixed fit: quadratic for XY-driving joints (shoulder_pan = 0,
    // wrist_roll = 4, gripper = 5) — these benefit from non-linear fit
    // (lens curvature). Height-driving joints (1, 2, 3) ALWAYS use affine,
    // because quadratic on those over-curves and can dive into the table.
    const HEIGHT_JOINTS = new Set([1, 2, 3]);
    const mixed = (
      quad: ReturnType<typeof applyQuadratics> | null,
      aff: number[],
    ): number[] => {
      if (!quad) return aff;
      return quad.map((v, i) => (HEIGHT_JOINTS.has(i) ? aff[i] : v));
    };
    const hoverAt = (px: number, py: number) => {
      const aff = applyAffines(calibration.affines!, px, py);
      const quad = calibration.quadratics
        ? applyQuadratics(calibration.quadratics, px, py)
        : null;
      return mixed(quad, aff);
    };
    const pickupAt = (px: number, py: number) => {
      if (!calibration.pickupAffines) return hoverAt(px, py);
      const aff = applyAffines(calibration.pickupAffines, px, py);
      const quad = calibration.pickupQuadratics
        ? applyQuadratics(calibration.pickupQuadratics, px, py)
        : null;
      return mixed(quad, aff);
    };

    const lerp = (a: number[], b: number[], t: number) =>
      a.map((v, i) => Math.round(v + (b[i] - v) * t));
    /** Descent target. Computes the raw target then freezes shoulder_pan
     *  (joint 0) and wrist_roll (joint 4) at their hover values, so the
     *  descent stays in the arm's vertical plane and the gripper tip
     *  doesn't sweep laterally in the camera frame.
     *
     *  Per-pixel (pickup affine) is preferred over the global descentDelta
     *  because the arm geometry is non-linear: the same joint delta
     *  produces different XY motion at different workspace positions. */
    const descendTarget = (px: number, py: number) => {
      const hover = hoverAt(px, py);
      let raw: number[];
      const haveDelta = !!(calibration.descentDelta && calibration.descentDelta.length === 6);
      const havePickup = !!(calibration.pickupAffines && calibration.pickupSamples.length >= 3);
      const usePickup = (forceGlobalDescent && !haveDelta) ? havePickup : (havePickup && !forceGlobalDescent);
      if (usePickup) {
        raw = lerp(hover, pickupAt(px, py), descentDepth);
      } else if (haveDelta) {
        const delta = calibration.descentDelta!;
        raw = hover.map((v, i) => Math.round(v + descentDepth * delta[i]));
      } else {
        raw = hover;
      }
      // Endpoint adjustments to "coil" the arm during descent. Added on
      // top of the captured descent target so the user can dial in extra
      // elbow / wrist fold and reduce shoulder lift without recapturing.
      raw[1] += liftExtra;
      raw[2] += elbowExtra;
      raw[3] += wristExtra;
      // Freeze pan + roll to hover values to suppress lateral drift.
      raw[0] = hover[0];
      raw[4] = hover[4];
      return raw;
    };

    // Effective centroid for tracking. Use the live detection only if it's
    // confident (full-block area, not a partial-occlusion sliver). Otherwise
    // fall back to the last trusted centroid — once we've ever seen the
    // block, we keep that position because the block isn't moving on its own.
    const liveConfident =
      blockTracked && blockTracked.count >= recentMaxBlockArea * confidenceFloor;
    const effectiveBlock: { cx: number; cy: number } | null = liveConfident
      ? { cx: blockTracked!.cx, cy: blockTracked!.cy }
      : lastBlockCentroid
      ? { cx: lastBlockCentroid.x, cy: lastBlockCentroid.y }
      : null;

    const currentJoints = $followerJoints.positions;

    switch (pnpState) {
      case 'idle': {
        if (!effectiveBlock) {
          const idleFor = now - lastTrackTime;
          if (idleFor > 800) trackingStatus = 'no block found';
          return null;
        }
        lastTrackTime = now;
        if (!insideHull(calibration.samples, effectiveBlock.cx, effectiveBlock.cy)) {
          trackingStatus = 'block outside calibration area';
          return null;
        }
        // Hover offset keeps the gripper out of the camera's line of sight
        // to the block, so we don't lose the centroid during approach.
        const hx = effectiveBlock.cx + hoverOffsetX;
        const hy = effectiveBlock.cy + hoverOffsetY;
        const affineTargets = hoverAt(hx, hy);
        const targets = affineTargets.slice();
        if (gripperMode === 'closed') targets[5] = gripperClosedRaw;
        else if (gripperMode === 'open') targets[5] = gripperOpenRaw;
        const tag = gripperMode === 'closed' ? 'gripping' : gripperMode === 'open' ? 'releasing' : 'tracking';
        return {
          targets,
          affineTargets,
          label: `${tag} → (${Math.round(effectiveBlock.cx)}, ${Math.round(effectiveBlock.cy)})${blockTracked ? '' : ' (held)'}`,
        };
      }

      case 'await_block': {
        if (!effectiveBlock) return { ...holdTargets(), label: 'pnp: waiting for block' };
        if (!insideHull(calibration.samples, effectiveBlock.cx, effectiveBlock.cy)) {
          return { ...holdTargets(), label: 'pnp: block out of bounds' };
        }
        // Same offset-hover as idle so the gripper sits beside the block
        // while we wait for centroid stability. The frozen target captured
        // when stability fires is the *true* centroid (no offset), so
        // descent goes straight down on the block.
        const hx = effectiveBlock.cx + hoverOffsetX;
        const hy = effectiveBlock.cy + hoverOffsetY;
        const affineTargets = hoverAt(hx, hy);
        // Only feed the stability detector with a real fresh detection, but
        // don't reset the timer just because the gripper briefly covered it.
        if (blockTracked) {
          updateStability(blockTracked, currentJoints, affineTargets, true);
        }
        if (stableSinceMs && now - stableSinceMs >= STABILITY_WINDOW_MS) {
          // Median centroid over the stability window — robust to occlusion-
          // biased frames where the gripper partially covers the block.
          const xs = centroidHistory.map((p) => p.x).slice().sort((a, b) => a - b);
          const ys = centroidHistory.map((p) => p.y).slice().sort((a, b) => a - b);
          const mid = Math.floor(xs.length / 2);
          const mx = xs[mid] ?? effectiveBlock.cx;
          const my = ys[mid] ?? effectiveBlock.cy;
          pnpFrozenPx = { x: mx + pickBiasX, y: my + pickBiasY };
          enterPnp('descend_pick');
        }
        const targets = affineTargets.slice();
        targets[5] = gripperOpenRaw;
        return {
          targets,
          affineTargets,
          label: `pnp: stabilizing (${Math.round(stabilityProgress * 100)}%)${blockTracked ? '' : ' (held)'}`,
        };
      }

      case 'descend_pick': {
        descentDepth = pickDepth;

        // Visual-servoed pixel target. Initialize smoothed pixel from frozen
        // on first tick, then low-pass toward the live centroid when it's
        // both detected AND within a sane radius of the frozen target
        // (rejects occlusion-biased outliers).
        if (!smoothedPickPx) smoothedPickPx = { ...pnpFrozenPx! };
        if (liveCorrectDescent && blockTracked) {
          const lx = blockTracked.cx + pickBiasX;
          const ly = blockTracked.cy + pickBiasY;
          const dx = lx - pnpFrozenPx!.x;
          const dy = ly - pnpFrozenPx!.y;
          if (Math.hypot(dx, dy) < liveCorrectRadius) {
            smoothedPickPx = {
              x: smoothedPickPx.x + liveCorrectAlpha * (lx - smoothedPickPx.x),
              y: smoothedPickPx.y + liveCorrectAlpha * (ly - smoothedPickPx.y),
            };
          }
        }

        const px = smoothedPickPx;
        const targets = descendTarget(px.x + pickDescBiasX, px.y + pickDescBiasY);
        targets[5] = gripperOpenRaw;
        const arrived = arrivedAt(targets, currentJoints, 5);
        if ((arrived && elapsed > PNP_MIN_DWELL_MS) || elapsed > PNP_MOVE_TIMEOUT_MS) {
          // Lock the corrected pixel into the frozen target so close_pick /
          // lift_pick continue to operate on the corrected position.
          pnpFrozenPx = { x: smoothedPickPx.x, y: smoothedPickPx.y };
          enterPnp('close_pick');
        }
        return { targets, affineTargets: targets, label: `pnp: descending${blockTracked ? ' (servo)' : ' (held)'}`, maxStep: pnpStep };
      }

      case 'close_pick': {
        descentDepth = pickDepth;
        const px = pnpFrozenPx!;
        const targets = descendTarget(px.x + pickDescBiasX, px.y + pickDescBiasY);
        targets[5] = gripperClosedRaw;
        const gripperArrived = Math.abs((currentJoints[5] ?? 0) - gripperClosedRaw) < 80;
        const minClose = elapsed >= PNP_GRIP_HOLD_MS;
        if ((gripperArrived && minClose) || elapsed >= PNP_MOVE_TIMEOUT_MS) {
          enterPnp('lift_pick');
        }
        return { targets, affineTargets: targets, label: 'pnp: closing gripper', maxStep: pnpStep };
      }

      case 'lift_pick': {
        descentDepth = pickDepth;
        const px = pnpFrozenPx!;
        const targets = hoverAt(px.x, px.y);
        targets[5] = gripperClosedRaw;
        const arrived = arrivedAt(targets, currentJoints, 5);
        if ((arrived && elapsed > PNP_MIN_DWELL_MS) || elapsed > PNP_MOVE_TIMEOUT_MS) {
          if (!drop) { abortPnp('no drop spot set'); return holdAndLabel('aborted'); }
          enterPnp('move_to_drop');
        }
        return { targets, affineTargets: targets, label: `pnp: lifting`, maxStep: pnpStep };
      }

      case 'move_to_drop': {
        if (!drop) { abortPnp('no drop spot'); return holdAndLabel('aborted'); }
        const targets = hoverAt(drop.px, drop.py);
        targets[5] = gripperClosedRaw;
        const arrived = arrivedAt(targets, currentJoints, 5);
        if ((arrived && elapsed > PNP_MIN_DWELL_MS) || elapsed > PNP_MOVE_TIMEOUT_MS) enterPnp('await_drop');
        return { targets, affineTargets: targets, label: `pnp: moving to drop`, maxStep: pnpStep };
      }

      case 'await_drop': {
        if (!drop) { abortPnp('no drop spot'); return holdAndLabel('aborted'); }
        const targets = hoverAt(drop.px, drop.py);
        targets[5] = gripperClosedRaw;
        if (elapsed >= 300) {
          pnpFrozenPx = { x: drop.px, y: drop.py };
          enterPnp('descend_drop');
        }
        return { targets, affineTargets: targets, label: 'pnp: settling over drop', maxStep: pnpStep };
      }

      case 'descend_drop': {
        descentDepth = dropDepth;
        const dropPx = pnpFrozenPx!;
        // Live drop correction: the block is in the gripper and visible,
        // so its centroid is approximately where the gripper is in image
        // space. Compute error vs the drop target and nudge the commanded
        // pixel to push the block onto the drop spot.
        if (!smoothedDropPx) smoothedDropPx = { ...dropPx };
        if (liveCorrectDescent && blockTracked && blockTracked.count >= recentMaxBlockArea * confidenceFloor) {
          const errX = dropPx.x - blockTracked.cx;
          const errY = dropPx.y - blockTracked.cy;
          const newX = smoothedDropPx.x + liveCorrectAlpha * errX;
          const newY = smoothedDropPx.y + liveCorrectAlpha * errY;
          // Clamp correction to within liveCorrectRadius of the original drop
          if (Math.hypot(newX - dropPx.x, newY - dropPx.y) < liveCorrectRadius) {
            smoothedDropPx = { x: newX, y: newY };
          }
        }
        const px = smoothedDropPx;
        const targets = descendTarget(px.x + dropDescBiasX, px.y + dropDescBiasY);
        targets[5] = gripperClosedRaw;
        const arrived = arrivedAt(targets, currentJoints, 5);
        if ((arrived && elapsed > PNP_MIN_DWELL_MS) || elapsed > PNP_MOVE_TIMEOUT_MS) enterPnp('open_drop');
        return { targets, affineTargets: targets, label: `pnp: descending to drop${blockTracked ? ' (servo)' : ' (held)'}`, maxStep: pnpStep };
      }

      case 'open_drop': {
        descentDepth = dropDepth;
        // Use the corrected drop pixel from live-servo descent if available.
        const px = smoothedDropPx ?? pnpFrozenPx!;
        const targets = descendTarget(px.x + dropDescBiasX, px.y + dropDescBiasY);
        targets[5] = gripperOpenRaw;
        const gripperArrived = Math.abs((currentJoints[5] ?? 0) - gripperOpenRaw) < 80;
        const minRelease = elapsed >= PNP_RELEASE_HOLD_MS;
        if ((gripperArrived && minRelease) || elapsed >= PNP_MOVE_TIMEOUT_MS) {
          enterPnp('lift_drop');
        }
        return { targets, affineTargets: targets, label: 'pnp: releasing', maxStep: pnpStep };
      }

      case 'lift_drop': {
        descentDepth = dropDepth;
        const px = pnpFrozenPx!;
        const targets = hoverAt(px.x, px.y);
        targets[5] = gripperOpenRaw;
        const arrived = arrivedAt(targets, currentJoints, 5);
        if ((arrived && elapsed > PNP_MIN_DWELL_MS) || elapsed > PNP_MOVE_TIMEOUT_MS) {
          gripperMode = 'auto';
          enterPnp('idle');
        }
        return { targets, affineTargets: targets, label: `pnp: lifting from drop`, maxStep: pnpStep };
      }
    }
    return null;
  }

  function holdTargets(): { targets: number[]; affineTargets: number[] } {
    const last = lastSentTargets ?? $followerJoints.positions.slice();
    return { targets: last.slice(), affineTargets: last.slice() };
  }

  function holdAndLabel(label: string) {
    return { ...holdTargets(), label: `pnp: ${label}` };
  }

  function enterPnp(s: PnPState) {
    pnpState = s;
    pnpStateEnteredMs = Date.now();
    if (s === 'idle') pnpFrozenPx = null;
    if (s !== 'descend_pick') smoothedPickPx = null;
    if (s !== 'descend_drop' && s !== 'open_drop') smoothedDropPx = null;
  }

  function startPickAndPlace() {
    if (!pickAndPlaceReady) {
      errorMessage = 'Need hover + pickup calibration and a drop spot before running pick-and-place.';
      return;
    }
    if (!followerTrackingEnabled) {
      errorMessage = 'Enable follower tracking first.';
      return;
    }
    errorMessage = '';
    gripperMode = 'auto';
    autoGrabArmed = false;
    enterPnp('await_block');
  }

  function abortPnp(reason: string) {
    enterPnp('idle');
    gripperMode = 'auto';
    trackingStatus = `pnp aborted: ${reason}`;
  }

  function updateStability(
    block: TrackResult,
    currentJoints: number[],
    targetAffine: number[],
    active: boolean = autoGrabArmed,
  ): void {
    const now = Date.now();
    centroidHistory.push({ x: block.cx, y: block.cy, t: now });
    const cutoff = now - STABILITY_WINDOW_MS;
    while (centroidHistory.length && centroidHistory[0].t < cutoff) {
      centroidHistory.shift();
    }

    if (!active) {
      stableSinceMs = null;
      stabilityProgress = 0;
      return;
    }

    // Need at least the full window worth of samples
    if (centroidHistory.length < 3 || now - centroidHistory[0].t < STABILITY_WINDOW_MS - 50) {
      stableSinceMs = null;
      stabilityProgress = 0;
      return;
    }

    let mx = 0, my = 0;
    for (const p of centroidHistory) { mx += p.x; my += p.y; }
    mx /= centroidHistory.length;
    my /= centroidHistory.length;
    let maxDev = 0;
    for (const p of centroidHistory) {
      const d = Math.hypot(p.x - mx, p.y - my);
      if (d > maxDev) maxDev = d;
    }

    // Centroid-only stability — joint arrival is too strict because the
    // affine target moves with every centroid pixel of jitter, so the
    // controller never quite catches up. Visual stability is sufficient.
    void targetAffine;
    void currentJoints;
    const stable = maxDev < stabilityPixelRadius;
    if (stable) {
      if (stableSinceMs === null) stableSinceMs = now;
      const elapsed = now - stableSinceMs;
      stabilityProgress = Math.min(1, elapsed / STABILITY_WINDOW_MS);
      if (elapsed >= STABILITY_WINDOW_MS && autoGrabArmed) {
        // Auto-grab toggle (separate from pick-and-place which checks
        // stableSinceMs itself before transitioning state)
        gripperMode = 'closed';
        autoGrabArmed = false;
        stableSinceMs = null;
        stabilityProgress = 0;
      }
    } else {
      stableSinceMs = null;
      stabilityProgress = 0;
    }
  }

  function armAutoGrab() {
    if (gripperMode === 'closed') {
      // Cancel grip
      gripperMode = 'auto';
      autoGrabArmed = false;
      return;
    }
    autoGrabArmed = !autoGrabArmed;
    stableSinceMs = null;
    stabilityProgress = 0;
  }

  function startRelease() {
    prevGripperMode = gripperMode;
    gripperMode = 'open';
  }

  function endRelease() {
    if (gripperMode !== 'open') return;
    // After dropping, return to free (auto) — user can rearm to grab again.
    gripperMode = 'auto';
    autoGrabArmed = false;
    stableSinceMs = null;
    stabilityProgress = 0;
    void prevGripperMode;
  }

  async function toggleFollowerTracking() {
    if (followerTrackingEnabled) {
      followerTrackingEnabled = false;
      lastSentTargets = null;
      lastBlockCentroid = null;
      recentMaxBlockArea = 0;
      trackingStatus = 'stopped';
      return;
    }
    if (!calibrationReady) {
      errorMessage = 'Capture at least 3 calibration samples first.';
      return;
    }
    if (!$followerConnection?.connected) {
      errorMessage = 'Connect the follower arm before tracking.';
      return;
    }
    if ($isMirroring) {
      errorMessage = 'Stop mirroring before enabling follower tracking.';
      return;
    }
    errorMessage = '';
    // Ensure torque is on — calibration / descent capture detorques the
    // follower, and position commands silently no-op without torque.
    try {
      await setTorque('follower', true);
    } catch (e) {
      errorMessage = `Failed to enable torque: ${e}`;
      return;
    }
    lastSentTargets = null;
    followerTrackingEnabled = true;
    trackingStatus = 'starting';
  }

  onMount(() => {
    const saved = loadCalibrationFromLocalStorage();
    if (saved && saved.samples?.length) {
      calibration = saved;
      if (!calibration.affines) calibration.affines = fitAffines(calibration.samples);
    }

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
      followerTrackingEnabled = false;
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

  <div class="video-wrap" class:picking={pickMode || blockPickMode || calibrationClickArm}>
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

  <!-- Primary actions: always visible -->
  <div class="primary-row">
    <button
      class="big-btn"
      class:active={followerTrackingEnabled}
      on:click={toggleFollowerTracking}
    >
      {followerTrackingEnabled ? 'Stop tracking' : 'Track block'}
    </button>

    <button
      class="big-btn primary"
      on:click={startPickAndPlace}
      disabled={!pickAndPlaceReady || !followerTrackingEnabled || pnpState !== 'idle'}
    >
      {pnpState !== 'idle' ? `PnP: ${pnpState}` : 'Run pick-and-place'}
    </button>

    {#if pnpState !== 'idle'}
      <button class="big-btn release" on:click={() => abortPnp('user abort')}>Abort</button>
    {/if}

    <button
      class="big-btn"
      class:active={autoGrabArmed || gripperMode === 'closed'}
      on:click={armAutoGrab}
      disabled={!followerTrackingEnabled}
    >
      {gripperMode === 'closed' ? 'Gripping (cancel)' : autoGrabArmed ? 'Watching...' : 'Grab'}
    </button>

    <button
      class="big-btn release"
      class:active={gripperMode === 'open'}
      on:pointerdown={startRelease}
      on:pointerup={endRelease}
      on:pointerleave={endRelease}
      on:pointercancel={endRelease}
      disabled={!followerTrackingEnabled && gripperMode !== 'open'}
    >
      Release (hold)
    </button>

    {#if autoGrabArmed && stabilityProgress > 0}
      <div class="stab-bar"><div class="stab-fill" style="width: {Math.round(stabilityProgress * 100)}%"></div></div>
    {/if}
  </div>

  <!-- Compact status -->
  <div class="status-line">
    {#if pnpState !== 'idle' || followerTrackingEnabled}
      <span>{trackingStatus || (followerTrackingEnabled ? 'tracking' : 'idle')}</span>
    {/if}
    <span class="dim">
      cal: {calibration.samples.length}H/{calibration.pickupSamples.length}P · drop: {calibration.dropSpot ? 'set' : '—'}
      {#if !pickAndPlaceReady && !calibrationMode}
        ({!calibrationReady ? 'need hover' : !descentReady ? 'need pickup/descent' : 'need drop'})
      {/if}
    </span>
  </div>

  <!-- ─── Block & calibration ─── -->
  <details open>
    <summary>Block &amp; calibration</summary>
    <div class="section">
      <div class="track-controls">
        <label class="color-picker" title="Block color the follower will chase">
          <input type="color" bind:value={blockColor} />
          <span class="hex">{blockColor.toUpperCase()}</span>
        </label>
        <button
          class="scan-btn"
          class:active={blockPickMode}
          on:click={() => { blockPickMode = !blockPickMode; pickMode = false; calibrationClickArm = false; }}
          disabled={!capturing}
        >
          {blockPickMode ? 'Click block in video...' : 'Pick block color'}
        </button>

        <span class="spacer-h"></span>

        {#if !calibrationMode}
          <button class="scan-btn" on:click={startCalibration}>
            {calibration.samples.length > 0 ? 'Re-calibrate' : 'Calibrate'}
          </button>
          {#if calibration.samples.length > 0}
            <button class="scan-btn" on:click={clearCalibration}>Clear</button>
          {/if}
        {:else}
          <div class="pass-toggle">
            <button
              class="scan-btn"
              class:active={calibrationPass === 'hover'}
              on:click={() => (calibrationPass = 'hover')}
            >Hover</button>
            <button
              class="scan-btn"
              class:active={calibrationPass === 'pickup'}
              on:click={() => (calibrationPass = 'pickup')}
            >Pickup</button>
          </div>
          <button
            class="scan-btn"
            class:active={calibrationClickArm}
            on:click={() => { calibrationClickArm = !calibrationClickArm; pickMode = false; blockPickMode = false; dropSpotClickArm = false; }}
            disabled={!$followerConnection?.connected}
          >
            {calibrationClickArm ? 'Click gripper tip...' : `Capture ${calibrationPass}`}
          </button>
          <button class="scan-btn" on:click={removeLastSample}>Undo</button>
          <button class="scan-btn" on:click={finishCalibration} disabled={calibration.samples.length < 3}>Done</button>
        {/if}

        <button
          class="scan-btn"
          class:active={dropSpotClickArm}
          on:click={setDropSpot}
        >
          {dropSpotClickArm ? 'Click drop spot...' : calibration.dropSpot ? 'Move drop' : 'Set drop'}
        </button>
      </div>

      {#if calibrationMode}
        <div class="calib-help">
          {#if calibrationPass === 'hover'}
            <strong>Hover pass:</strong> detorque the follower. Place the gripper tip above
            each point at the safe Z (≥ 8 cm above table), click <strong>Capture hover</strong>,
            then click the tip location in the video. 6+ points spread across the workspace
            unlocks the quadratic fit (better accuracy with fisheye lenses).
          {:else}
            <strong>Pickup pass:</strong> at the same XYs as hover, lower the gripper to grasp
            height. Click <strong>Capture pickup</strong>, then click the tip in the video.
          {/if}
        </div>
      {/if}

      <span class="cal-label dim">
        fit · hover: {calibration.quadratics ? `quad ${calibration.samples.length}pt` : calibration.affines ? `affine ${calibration.samples.length}pt` : '—'}
        · pickup: {calibration.pickupQuadratics ? `quad ${calibration.pickupSamples.length}pt` : pickupReady ? `affine ${calibration.pickupSamples.length}pt` : calibration.descentDelta ? 'global Δ' : '—'}
        · height: affine (locked)
      </span>
    </div>
  </details>

  <!-- ─── Tuning ─── -->
  <details>
    <summary>Tuning</summary>
    <div class="section">
      <div class="tuning-grid">
        <label class="slider"><span>Pick depth ×{pickDepth.toFixed(2)}</span>
          <input type="range" min="0.5" max="1.6" step="0.01" bind:value={pickDepth} /></label>
        <label class="slider"><span>Drop depth ×{dropDepth.toFixed(2)}</span>
          <input type="range" min="0.5" max="1.6" step="0.01" bind:value={dropDepth} /></label>
        <label class="slider"><span>Arm speed {pnpStep}</span>
          <input type="range" min="10" max="120" step="1" bind:value={pnpStep} /></label>
        <label class="slider"><span>Stability jitter ±{stabilityPixelRadius}px</span>
          <input type="range" min="3" max="40" step="1" bind:value={stabilityPixelRadius} /></label>

        <label class="slider"><span>Pick desc X {pickDescBiasX}px</span>
          <input type="range" min="-300" max="300" step="1" bind:value={pickDescBiasX} /></label>
        <label class="slider"><span>Pick desc Y {pickDescBiasY}px</span>
          <input type="range" min="-300" max="300" step="1" bind:value={pickDescBiasY} /></label>
        <label class="slider"><span>Drop desc X {dropDescBiasX}px</span>
          <input type="range" min="-300" max="300" step="1" bind:value={dropDescBiasX} /></label>
        <label class="slider"><span>Drop desc Y {dropDescBiasY}px</span>
          <input type="range" min="-300" max="300" step="1" bind:value={dropDescBiasY} /></label>

        <label class="slider"><span>Bias X {pickBiasX}px</span>
          <input type="range" min="-50" max="50" step="1" bind:value={pickBiasX} /></label>
        <label class="slider"><span>Bias Y {pickBiasY}px</span>
          <input type="range" min="-50" max="50" step="1" bind:value={pickBiasY} /></label>

        <label class="slider" title="How quickly live correction pulls toward the live centroid each frame. 1 = instant, 0 = no correction.">
          <span>Live α {liveCorrectAlpha.toFixed(2)}</span>
          <input type="range" min="0" max="1" step="0.05" bind:value={liveCorrectAlpha} />
        </label>
        <label class="slider" title="Max pixel distance the live correction can pull the target from the original frozen point. Prevents occlusion-biased outliers.">
          <span>Live radius {liveCorrectRadius}px</span>
          <input type="range" min="20" max="400" step="5" bind:value={liveCorrectRadius} />
        </label>

        <label class="slider" title="Detection-area fraction below which the live centroid is ignored (treated as occluded). Higher = more strict, locks in earlier.">
          <span>Min visible {Math.round(confidenceFloor * 100)}%</span>
          <input type="range" min="0.1" max="0.95" step="0.05" bind:value={confidenceFloor} />
        </label>

        <label class="slider" title="Pixel offset for hover during tracking — shifts the gripper away from directly above so the camera keeps seeing the block. Negative Y = up in image (behind the arm for an overhead-front cam).">
          <span>Hover offset X {hoverOffsetX}px</span>
          <input type="range" min="-150" max="150" step="1" bind:value={hoverOffsetX} />
        </label>
        <label class="slider" title="Hover offset Y — try negative if your camera is angled above and the gripper enters from the bottom of the image.">
          <span>Hover offset Y {hoverOffsetY}px</span>
          <input type="range" min="-150" max="150" step="1" bind:value={hoverOffsetY} />
        </label>

        <label class="slider" title="Boosts elbow_flex step rate during descent. Endpoint unchanged. Higher = elbow folds in first; arm goes from horizontal-extended to folded-vertical posture.">
          <span>Elbow first × {elbowFirstBoost.toFixed(2)}</span>
          <input type="range" min="1" max="5" step="0.05" bind:value={elbowFirstBoost} />
        </label>
        <label class="slider" title="Boosts wrist_flex step rate during descent. Pair with Elbow first to fold the whole forearm + wrist before shoulder catches up.">
          <span>Wrist first × {wristFirstBoost.toFixed(2)}</span>
          <input type="range" min="1" max="5" step="0.05" bind:value={wristFirstBoost} />
        </label>

        <label class="slider" title="Adds raw counts to the SHOULDER_LIFT descent endpoint. Negative reduces lift (less arm extension). Use to coil the arm in.">
          <span>Lift extra {liftExtra}</span>
          <input type="range" min="-800" max="800" step="10" bind:value={liftExtra} />
        </label>
        <label class="slider" title="Adds raw counts to the ELBOW_FLEX descent endpoint. Positive folds the elbow more (coil up). Use bias X/Y to fix any XY drift.">
          <span>Elbow extra {elbowExtra}</span>
          <input type="range" min="-800" max="800" step="10" bind:value={elbowExtra} />
        </label>
        <label class="slider" title="Adds raw counts to the WRIST_FLEX descent endpoint. Positive bends the wrist more.">
          <span>Wrist extra {wristExtra}</span>
          <input type="range" min="-800" max="800" step="10" bind:value={wristExtra} />
        </label>

        <label class="slider"><span>Gripper closed {gripperClosedRaw}</span>
          <input type="range" min="0" max="4095" step="1" bind:value={gripperClosedRaw} /></label>
        <label class="slider"><span>Gripper open {gripperOpenRaw}</span>
          <input type="range" min="0" max="4095" step="1" bind:value={gripperOpenRaw} /></label>
        <label class="slider"><span>Grip speed {gripperStep}</span>
          <input type="range" min="5" max="100" step="1" bind:value={gripperStep} /></label>
      </div>

      <div class="track-controls">
        <label class="toggle" title="Use the captured global descent Δ instead of the per-pixel pickup fit. Try this if PnP descends but the arm doesn't move (pickup samples are probably equal to hover samples).">
          <input type="checkbox" bind:checked={forceGlobalDescent} />
          Force global Δ
        </label>

        <label class="toggle" title="During descent, low-pass the target pixel toward the live block centroid when visible. Off = one-shot frozen target.">
          <input type="checkbox" bind:checked={liveCorrectDescent} />
          Live correction
        </label>

        <span class="spacer-h"></span>

        <button
          class="scan-btn"
          class:active={descentCaptureStage === 'awaiting_low'}
          on:click={captureDescentStep}
          disabled={!$followerConnection?.connected}
        >
          {descentCaptureStage === 'idle'
            ? (calibration.descentDelta ? 'Re-capture descent Δ' : 'Capture descent Δ')
            : 'Now lower gripper, click again'}
        </button>
        {#if calibration.descentDelta}
          <button class="scan-btn" on:click={clearDescentDelta}>Clear Δ</button>
        {/if}
      </div>
    </div>
  </details>

  <!-- ─── Free color tracker (rare) ─── -->
  <details>
    <summary>Free color tracker (independent of follower)</summary>
    <div class="section">
      <div class="track-controls">
        <label class="toggle">
          <input type="checkbox" bind:checked={trackingEnabled} />
          Track color
        </label>
        <label class="color-picker">
          <input type="color" bind:value={targetColor} />
          <span class="hex">{targetColor.toUpperCase()}</span>
        </label>
        <button class="scan-btn" class:active={pickMode}
          on:click={() => (pickMode = !pickMode)} disabled={!capturing}>
          {pickMode ? 'Click on video...' : 'Pick from camera'}
        </button>
      </div>

      <div class="tuning-grid">
        <label class="slider"><span>Free hue ±{hueTolerance}°</span>
          <input type="range" min="2" max="60" step="1" bind:value={hueTolerance} /></label>
        <label class="slider"><span>Free sat ≥ {satMin.toFixed(2)}</span>
          <input type="range" min="0" max="1" step="0.05" bind:value={satMin} /></label>
        <label class="slider"><span>Free val ≥ {valMin.toFixed(2)}</span>
          <input type="range" min="0" max="1" step="0.05" bind:value={valMin} /></label>
        <label class="slider"><span>Block hue ±{blockHueTol}°</span>
          <input type="range" min="2" max="60" step="1" bind:value={blockHueTol} /></label>
        <label class="slider"><span>Block sat ≥ {blockSatMin.toFixed(2)}</span>
          <input type="range" min="0" max="1" step="0.05" bind:value={blockSatMin} /></label>
        <label class="slider"><span>Block val ≥ {blockValMin.toFixed(2)}</span>
          <input type="range" min="0" max="1" step="0.05" bind:value={blockValMin} /></label>
      </div>

      {#if trackingEnabled && tracked}
        <div class="track-info">
          <span>centroid: ({Math.round(tracked.cx)}, {Math.round(tracked.cy)})</span>
          <span>box: {tracked.w}×{tracked.h}</span>
          <span>pixels: {tracked.count}</span>
        </div>
      {/if}
    </div>
  </details>

  <!-- ─── Debug ─── -->
  <details>
    <summary>Debug</summary>
    <div class="section">
      <div class="track-info">
        <span>state: {pnpState}{pnpState !== 'idle' ? ` · ${Date.now() - pnpStateEnteredMs}ms` : ''}</span>
        {#if blockTracked}<span>block @ ({Math.round(blockTracked.cx)}, {Math.round(blockTracked.cy)})</span>{/if}
        {#if lastSentTargets}<span>sent: [{lastSentTargets.join(', ')}]</span>{/if}
        <span>cur: [{$followerJoints.positions.join(', ')}]</span>
        {#if calibration.descentDelta}<span>Δ: [{calibration.descentDelta.join(', ')}]</span>{/if}
        {#if lastSentTargets}
          <span>diff: [{$followerJoints.positions.map((c, i) => (lastSentTargets?.[i] ?? c) - c).join(', ')}]</span>
        {/if}
        {#if lastFrameAt}<span class="dim">last frame {lastFrameAt}</span>{/if}
      </div>
    </div>
  </details>
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

  h4 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    color: #c0d0e0;
  }

  .sep {
    border: none;
    border-top: 1px solid #1e2a3a;
    margin: 6px 0;
  }

  .cal-label {
    font-size: 12px;
    color: #8aa0b8;
    font-family: monospace;
  }

  .spacer-h {
    flex: 1;
  }

  .calib-help {
    padding: 8px 10px;
    background: #0d1320;
    border-left: 3px solid #2563eb;
    border-radius: 3px;
    font-size: 12px;
    color: #aabbcc;
    line-height: 1.5;
  }

  .calib-help strong {
    color: #e0e0e0;
  }

  .scan-btn.release {
    background: #3a1f1f;
    color: #fca5a5;
    border-color: #7f1d1d;
  }

  .scan-btn.release:hover:not(:disabled) {
    background: #4a2525;
    color: #fecaca;
  }

  .scan-btn.release.active {
    background: #7f1d1d;
    color: #fff;
  }

  .stab-bar {
    width: 100px;
    height: 6px;
    background: #1e2a3a;
    border-radius: 3px;
    overflow: hidden;
  }

  .stab-fill {
    height: 100%;
    background: linear-gradient(90deg, #2563eb, #4ade80);
    transition: width 80ms linear;
  }

  .pass-toggle {
    display: inline-flex;
    gap: 4px;
    border: 1px solid #1e2a3a;
    border-radius: 4px;
    padding: 2px;
  }

  .scan-btn.primary {
    background: #1e3a5f;
    color: #60a5fa;
    border-color: #2563eb;
  }

  .scan-btn.primary:hover:not(:disabled) {
    background: #1e4080;
  }

  .cal-label.dim {
    color: #6f8193;
  }

  /* Consolidated layout */
  .primary-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
    padding: 8px;
    background: #0d1320;
    border: 1px solid #1e2a3a;
    border-radius: 6px;
  }

  .big-btn {
    padding: 10px 16px;
    border: 1px solid #2a3444;
    border-radius: 5px;
    background: #111827;
    color: #c0d0e0;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.12s;
    white-space: nowrap;
  }
  .big-btn:hover:not(:disabled) {
    background: #1a2332;
    border-color: #3a4a5a;
  }
  .big-btn.active {
    background: #1e3a5f;
    color: #60a5fa;
    border-color: #2563eb;
  }
  .big-btn.primary {
    background: #1e3a5f;
    color: #93c5fd;
    border-color: #2563eb;
  }
  .big-btn.primary:hover:not(:disabled) {
    background: #1e4080;
  }
  .big-btn.release {
    background: #3a1f1f;
    color: #fca5a5;
    border-color: #7f1d1d;
  }
  .big-btn.release:hover:not(:disabled),
  .big-btn.release.active {
    background: #7f1d1d;
    color: #fff;
  }
  .big-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .status-line {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    padding: 4px 6px;
    font-size: 12px;
    color: #c0d0e0;
    font-family: monospace;
  }
  .status-line .dim {
    color: #6f8193;
  }

  details {
    border: 1px solid #1e2a3a;
    border-radius: 6px;
    background: #0a0e17;
  }
  details > summary {
    list-style: none;
    cursor: pointer;
    padding: 8px 12px;
    font-size: 13px;
    font-weight: 600;
    color: #aabbcc;
    user-select: none;
  }
  details > summary::-webkit-details-marker { display: none; }
  details > summary::before {
    content: '▸';
    display: inline-block;
    width: 14px;
    color: #6f8193;
    transition: transform 0.12s;
  }
  details[open] > summary::before {
    transform: rotate(90deg);
  }
  details > summary:hover {
    color: #e0e0e0;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 0 12px 12px 12px;
  }

  .tuning-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 8px 14px;
  }

  .tuning-grid .slider {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
    color: #8aa0b8;
  }

  .tuning-grid .slider span {
    flex: 0 0 auto;
    min-width: 110px;
    font-family: monospace;
    color: #c0d0e0;
  }

  .tuning-grid .slider input[type='range'] {
    flex: 1;
    min-width: 0;
  }
</style>
