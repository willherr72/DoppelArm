/**
 * Pixel-to-joint calibration for vision-driven follower tracking.
 *
 * The user captures N (>= 3) samples where each sample pairs a camera pixel
 * with the follower's raw joint positions when the gripper tip was at that
 * pixel location, hovering at the desired Z above the table. We fit a per-
 * joint affine map (px, py, 1) -> raw_joint via least squares. Tracking then
 * evaluates the map at the live pixel centroid and sends the result to the
 * follower.
 *
 * Why no kinematic IK: the URDF mesh visualization was tuned by hand and the
 * stick-figure FK doesn't perfectly match it. Going through pixel-space lets
 * the calibration absorb every frame mismatch in one shot, and the safe Z
 * floor is enforced by *where you held the gripper while calibrating*, not
 * by trusting a kinematic model.
 */

export interface VisionCalibSample {
  px: number;
  py: number;
  /** Raw joint positions (length 6) at the moment of capture. */
  joints: number[];
}

/** Per-joint affine: target = a*px + b*py + c. */
export interface JointAffine {
  a: number;
  b: number;
  c: number;
}

/** Per-joint quadratic: target = c0 + c1*x + c2*y + c3*x² + c4*xy + c5*y².
 *  Captures gentle non-linear distortion (e.g. webcam fisheye) better than
 *  an affine. Requires 6+ samples to fit. */
export interface JointQuadratic {
  c: [number, number, number, number, number, number];
}

export interface VisionCalibration {
  /** Hover-height samples: gripper at safe Z above the table. */
  samples: VisionCalibSample[];
  /** One affine per joint, length 6, fit from hover samples. Null until fit. */
  affines: JointAffine[] | null;
  /** Quadratic fit (preferred over affines when 6+ samples are present). */
  quadratics: JointQuadratic[] | null;
  /** Pickup-height samples: gripper tip on/just above the table at same XY as
   *  the hover samples. Used to compute the descent leg of pick-and-place. */
  pickupSamples: VisionCalibSample[];
  pickupAffines: JointAffine[] | null;
  pickupQuadratics: JointQuadratic[] | null;
  /** Designated drop pixel, or null if not set. */
  dropSpot: { px: number; py: number } | null;
  /** Optional global descent vector: per-joint raw delta added to the hover
   *  pose to drop the gripper to grasp height. Captured by recording joints
   *  at hover, then again with the gripper manually lowered to the table.
   *  When present, this replaces the pickup-affine descent in pick-and-place. */
  descentDelta: number[] | null;
  /** Image dimensions when captured (pixels). For sanity checks. */
  width: number;
  height: number;
}

export const NUM_JOINTS = 6;

export function emptyCalibration(width: number, height: number): VisionCalibration {
  return {
    samples: [],
    affines: null,
    quadratics: null,
    pickupSamples: [],
    pickupAffines: null,
    pickupQuadratics: null,
    dropSpot: null,
    descentDelta: null,
    width,
    height,
  };
}

/**
 * Solve a 3x3 linear system Ax = b. Returns null on singularity.
 * Used for the "exactly 3 samples" closed form so we don't have to pull a
 * matrix library for the common case.
 */
function solve3x3(A: number[][], b: number[]): number[] | null {
  const m = A.map((row, i) => [...row, b[i]]);
  for (let col = 0; col < 3; col++) {
    let pivot = col;
    for (let r = col + 1; r < 3; r++) {
      if (Math.abs(m[r][col]) > Math.abs(m[pivot][col])) pivot = r;
    }
    if (Math.abs(m[pivot][col]) < 1e-9) return null;
    [m[col], m[pivot]] = [m[pivot], m[col]];
    for (let r = 0; r < 3; r++) {
      if (r === col) continue;
      const f = m[r][col] / m[col][col];
      for (let c = col; c < 4; c++) m[r][c] -= f * m[col][c];
    }
  }
  return [m[0][3] / m[0][0], m[1][3] / m[1][1], m[2][3] / m[2][2]];
}

/**
 * Fit per-joint affine via the normal equations: (XᵀX) p = Xᵀ y, where
 * X is N×3 [px py 1] and y is the joint values across samples.
 * Works for N >= 3. Returns null if the X positions are degenerate.
 */
export function fitAffines(samples: VisionCalibSample[]): JointAffine[] | null {
  if (samples.length < 3) return null;

  // Build XᵀX (3x3) — same for every joint
  let sxx = 0, sxy = 0, sx = 0;
  let syy = 0, sy = 0, n = samples.length;
  for (const s of samples) {
    sxx += s.px * s.px;
    sxy += s.px * s.py;
    sx += s.px;
    syy += s.py * s.py;
    sy += s.py;
  }
  const XtX: number[][] = [
    [sxx, sxy, sx],
    [sxy, syy, sy],
    [sx,  sy,  n],
  ];

  const out: JointAffine[] = [];
  for (let j = 0; j < NUM_JOINTS; j++) {
    let xy = 0, yy = 0, y = 0;
    for (const s of samples) {
      const v = s.joints[j] ?? 0;
      xy += s.px * v;
      yy += s.py * v;
      y  += v;
    }
    const sol = solve3x3(XtX.map(r => [...r]), [xy, yy, y]);
    if (!sol) return null;
    out.push({ a: sol[0], b: sol[1], c: sol[2] });
  }
  return out;
}

export function applyAffines(affines: JointAffine[], px: number, py: number): number[] {
  return affines.map(a => Math.round(a.a * px + a.b * py + a.c));
}

/**
 * Solve a 6x6 linear system Ax = b in-place via Gaussian elimination with
 * partial pivoting. Returns null on singularity.
 */
function solve6x6(A: number[][], b: number[]): number[] | null {
  const m = A.map((row, i) => [...row, b[i]]);
  for (let col = 0; col < 6; col++) {
    let pivot = col;
    for (let r = col + 1; r < 6; r++) {
      if (Math.abs(m[r][col]) > Math.abs(m[pivot][col])) pivot = r;
    }
    if (Math.abs(m[pivot][col]) < 1e-9) return null;
    [m[col], m[pivot]] = [m[pivot], m[col]];
    for (let r = 0; r < 6; r++) {
      if (r === col) continue;
      const f = m[r][col] / m[col][col];
      for (let c = col; c < 7; c++) m[r][c] -= f * m[col][c];
    }
  }
  return Array.from({ length: 6 }, (_, i) => m[i][6] / m[i][i]);
}

/**
 * Fit per-joint quadratic via normal equations on basis [1, x, y, x², xy, y²].
 * Requires 6+ samples; with exactly 6 the fit is exact (and may oscillate),
 * with more samples it's a least-squares smooth fit.
 */
export function fitQuadratics(samples: VisionCalibSample[]): JointQuadratic[] | null {
  if (samples.length < 6) return null;

  // Basis vectors b_i = [1, x, y, x², xy, y²]
  const basis = (s: VisionCalibSample) => [
    1,
    s.px,
    s.py,
    s.px * s.px,
    s.px * s.py,
    s.py * s.py,
  ];

  // Build XᵀX (6x6) — same for every joint
  const XtX: number[][] = Array.from({ length: 6 }, () => Array(6).fill(0));
  for (const s of samples) {
    const b = basis(s);
    for (let i = 0; i < 6; i++) {
      for (let j = 0; j < 6; j++) XtX[i][j] += b[i] * b[j];
    }
  }

  const out: JointQuadratic[] = [];
  for (let j = 0; j < NUM_JOINTS; j++) {
    const Xty = Array(6).fill(0);
    for (const s of samples) {
      const b = basis(s);
      const v = s.joints[j] ?? 0;
      for (let i = 0; i < 6; i++) Xty[i] += b[i] * v;
    }
    const sol = solve6x6(XtX.map((r) => [...r]), Xty);
    if (!sol) return null;
    out.push({ c: [sol[0], sol[1], sol[2], sol[3], sol[4], sol[5]] });
  }
  return out;
}

export function applyQuadratics(
  quads: JointQuadratic[],
  px: number,
  py: number,
): number[] {
  return quads.map(({ c }) =>
    Math.round(
      c[0] + c[1] * px + c[2] * py + c[3] * px * px + c[4] * px * py + c[5] * py * py,
    ),
  );
}

/**
 * Check that (px, py) is inside the convex hull of the calibration samples.
 * Used to refuse extrapolation, which can swing joints into untested regions
 * and bash the gripper into the table or itself.
 */
export function insideHull(samples: VisionCalibSample[], px: number, py: number): boolean {
  if (samples.length < 3) return false;
  const pts = samples.map(s => ({ x: s.px, y: s.py }));
  const hull = convexHull(pts);
  return pointInPolygon(hull, px, py);
}

function convexHull(points: { x: number; y: number }[]): { x: number; y: number }[] {
  const pts = [...points].sort((a, b) => a.x - b.x || a.y - b.y);
  const cross = (
    o: { x: number; y: number },
    a: { x: number; y: number },
    b: { x: number; y: number },
  ) => (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);

  const lower: typeof pts = [];
  for (const p of pts) {
    while (lower.length >= 2 && cross(lower[lower.length - 2], lower[lower.length - 1], p) <= 0) {
      lower.pop();
    }
    lower.push(p);
  }
  const upper: typeof pts = [];
  for (let i = pts.length - 1; i >= 0; i--) {
    const p = pts[i];
    while (upper.length >= 2 && cross(upper[upper.length - 2], upper[upper.length - 1], p) <= 0) {
      upper.pop();
    }
    upper.push(p);
  }
  upper.pop();
  lower.pop();
  return lower.concat(upper);
}

function pointInPolygon(poly: { x: number; y: number }[], px: number, py: number): boolean {
  let inside = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const xi = poly[i].x, yi = poly[i].y;
    const xj = poly[j].x, yj = poly[j].y;
    const intersects = ((yi > py) !== (yj > py))
      && (px < ((xj - xi) * (py - yi)) / (yj - yi + 1e-12) + xi);
    if (intersects) inside = !inside;
  }
  return inside;
}

const STORAGE_KEY = 'doppelarm.visionCalibration';

export function saveCalibrationToLocalStorage(c: VisionCalibration): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(c));
  } catch {
    // best-effort
  }
}

export function loadCalibrationFromLocalStorage(): VisionCalibration | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<VisionCalibration>;
    // Migrate older payloads that predate pickup/drop fields
    return {
      samples: parsed.samples ?? [],
      affines: parsed.affines ?? null,
      quadratics: parsed.quadratics ?? null,
      pickupSamples: parsed.pickupSamples ?? [],
      pickupAffines: parsed.pickupAffines ?? null,
      pickupQuadratics: parsed.pickupQuadratics ?? null,
      dropSpot: parsed.dropSpot ?? null,
      descentDelta: parsed.descentDelta ?? null,
      width: parsed.width ?? 640,
      height: parsed.height ?? 480,
    };
  } catch {
    return null;
  }
}
