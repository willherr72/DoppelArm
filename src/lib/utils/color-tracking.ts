export interface Hsv {
  h: number; // 0..360
  s: number; // 0..1
  v: number; // 0..1
}

export interface TrackResult {
  cx: number;
  cy: number;
  x: number;
  y: number;
  w: number;
  h: number;
  count: number;
}

export interface TrackOptions {
  hueTolerance: number; // degrees, applied symmetrically
  satMin: number; // 0..1
  valMin: number; // 0..1
  minPixels: number;
  step: number; // pixel stride for sampling (1 = every pixel)
}

export function rgbToHsv(r: number, g: number, b: number): Hsv {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const d = max - min;
  let h = 0;
  if (d !== 0) {
    if (max === rn) h = ((gn - bn) / d) % 6;
    else if (max === gn) h = (bn - rn) / d + 2;
    else h = (rn - gn) / d + 4;
    h *= 60;
    if (h < 0) h += 360;
  }
  const s = max === 0 ? 0 : d / max;
  return { h, s, v: max };
}

export function hexToRgb(hex: string): { r: number; g: number; b: number } {
  const v = hex.replace('#', '');
  const n = parseInt(v.length === 3
    ? v.split('').map((c) => c + c).join('')
    : v, 16);
  return { r: (n >> 16) & 0xff, g: (n >> 8) & 0xff, b: n & 0xff };
}

export function rgbToHex(r: number, g: number, b: number): string {
  const h = (n: number) => n.toString(16).padStart(2, '0');
  return `#${h(r)}${h(g)}${h(b)}`;
}

function hueDistance(a: number, b: number): number {
  const d = Math.abs(a - b);
  return d > 180 ? 360 - d : d;
}

export function findColor(
  data: Uint8ClampedArray,
  width: number,
  height: number,
  target: Hsv,
  opts: TrackOptions
): TrackResult | null {
  const { hueTolerance, satMin, valMin, minPixels, step } = opts;
  let sumX = 0;
  let sumY = 0;
  let count = 0;
  let minX = width;
  let minY = height;
  let maxX = -1;
  let maxY = -1;

  // For low-saturation targets, hue is unstable — match by value/saturation only.
  const useHue = target.s >= 0.15;

  for (let y = 0; y < height; y += step) {
    for (let x = 0; x < width; x += step) {
      const i = (y * width + x) * 4;
      const hsv = rgbToHsv(data[i], data[i + 1], data[i + 2]);
      if (hsv.s < satMin || hsv.v < valMin) continue;
      if (useHue && hueDistance(hsv.h, target.h) > hueTolerance) continue;
      if (!useHue) {
        if (Math.abs(hsv.v - target.v) > 0.2) continue;
        if (Math.abs(hsv.s - target.s) > 0.3) continue;
      }
      sumX += x;
      sumY += y;
      count++;
      if (x < minX) minX = x;
      if (y < minY) minY = y;
      if (x > maxX) maxX = x;
      if (y > maxY) maxY = y;
    }
  }

  if (count < minPixels) return null;
  return {
    cx: sumX / count,
    cy: sumY / count,
    x: minX,
    y: minY,
    w: maxX - minX,
    h: maxY - minY,
    count
  };
}
