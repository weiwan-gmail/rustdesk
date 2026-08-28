// Pure helpers for the web client's YUV → RGBA paint path. Kept free of
// browser / protobuf imports so node:test can lock the black-screen bugs:
//   1. WebGL attach already owns the canvas context; a second
//      getContext("webgl") without matching attributes returns null, and
//      draw() then takes neither the worker nor the readPixels branch.
//   2. Flutter decodeImageFromPixels uses PeerInfo display size, which can
//      be 0 or a default 1080x720 while the RGBA buffer is the real frame.
//   3. ogv.js worker frameBuffer is yuv-buffer shaped, but some fields can
//      be missing after transfer; drawFrame then leaves the canvas 0x0.

export type YuvPlane = { bytes: Uint8Array; stride: number };

export type YuvFormat = {
  width: number;
  height: number;
  chromaWidth: number;
  chromaHeight: number;
  cropLeft: number;
  cropTop: number;
  cropWidth: number;
  cropHeight: number;
  displayWidth: number;
  displayHeight: number;
};

export type YuvFrame = {
  format: YuvFormat;
  y: YuvPlane;
  u: YuvPlane;
  v: YuvPlane;
};

export type DrawBackend = "webgl" | "worker" | "software" | "none";

// Mirrors the pre-fix globals.draw() branch choice. When WebGLFrameSink
// is available the worker is never created; if attach()'s context then
// makes a bare getContext("webgl") return null, this is "none" — black.
export function legacySelectDrawBackend(opts: {
  yuvWorker: unknown;
  gl: unknown;
}): DrawBackend {
  if (opts.yuvWorker) return "worker";
  if (opts.gl) return "webgl";
  return "none";
}

// Never drop a decoded frame: software I420→RGBA always works.
export function selectDrawBackend(opts: {
  yuvWorker: unknown;
  gl: unknown;
}): DrawBackend {
  if (opts.yuvWorker) return "worker";
  if (opts.gl) return "webgl";
  return "software";
}

export function ogvDecoderClassName(codec: "vp9" | "vp8", isSIMD: boolean): string {
  if (codec == "vp9") {
    return isSIMD ? "OGVDecoderVideoVP9SIMDW" : "OGVDecoderVideoVP9W";
  }
  return "OGVDecoderVideoVP8W";
}

// threading:true needs SharedArrayBuffer (COOP/COEP). web-direct does not
// send those headers; the MT path then fails decoder init and every frame
// is queued forever.
export function ogvLoaderOptions(): { worker: boolean } {
  return { worker: true };
}

// ogv worker processFrame may call the callback with no args. Treat a
// present frameBuffer as success unless the decoder explicitly said no.
export function shouldPaintDecodedFrame(ok: unknown, frameBuffer: unknown): boolean {
  if (!frameBuffer) return false;
  return ok !== false;
}

function asPlane(plane: unknown, fallbackStride: number): YuvPlane | undefined {
  if (!plane) return undefined;
  if (plane instanceof Uint8Array) {
    return { bytes: plane, stride: fallbackStride };
  }
  const p = plane as { bytes?: unknown; stride?: unknown };
  if (p.bytes instanceof Uint8Array) {
    const stride = Number(p.stride);
    return { bytes: p.bytes, stride: stride > 0 ? stride : fallbackStride };
  }
  return undefined;
}

function asPositive(n: unknown, fallback: number): number {
  const v = Number(n);
  return Number.isFinite(v) && v > 0 ? v : fallback;
}

function asNonNeg(n: unknown, fallback: number): number {
  const v = Number(n);
  return Number.isFinite(v) && v >= 0 ? v : fallback;
}

// Accept ogv.js frameBuffer (yuv-buffer) and fill any fields the worker
// proxy dropped so yuv-canvas drawFrame / software convert have sizes.
export function normalizeOgvFrame(frame: unknown): YuvFrame | undefined {
  if (!frame || typeof frame !== "object") return undefined;
  const f = frame as {
    format?: Record<string, unknown>;
    y?: unknown;
    u?: unknown;
    v?: unknown;
  };
  const fmt = f.format && typeof f.format === "object" ? f.format : {};
  const width = asPositive(fmt.width, 0);
  const height = asPositive(fmt.height, 0);
  if (!width || !height) return undefined;
  const chromaWidth = asPositive(fmt.chromaWidth, Math.max(1, width >> 1));
  const chromaHeight = asPositive(fmt.chromaHeight, Math.max(1, height >> 1));
  const cropLeft = asNonNeg(fmt.cropLeft, 0);
  const cropTop = asNonNeg(fmt.cropTop, 0);
  const cropWidth = asPositive(fmt.cropWidth, width);
  const cropHeight = asPositive(fmt.cropHeight, height);
  const displayWidth = asPositive(fmt.displayWidth, cropWidth);
  const displayHeight = asPositive(fmt.displayHeight, cropHeight);
  const y = asPlane(f.y, width);
  const u = asPlane(f.u, chromaWidth);
  const v = asPlane(f.v, chromaWidth);
  if (!y || !u || !v) return undefined;
  return {
    format: {
      width,
      height,
      chromaWidth,
      chromaHeight,
      cropLeft,
      cropTop,
      cropWidth,
      cropHeight,
      displayWidth,
      displayHeight,
    },
    y,
    u,
    v,
  };
}

function clamp8(n: number): number {
  return n < 0 ? 0 : n > 255 ? 255 : n;
}

// JPEG/full-range YUV420 (same intent as yuv.js yuv420_rgb24_std(..., 1)).
// Crops to displayWidth x displayHeight.
export function i420ToRgba(frame: YuvFrame): Uint8Array | undefined {
  const { format, y, u, v } = frame;
  const w = format.displayWidth;
  const h = format.displayHeight;
  if (w <= 0 || h <= 0) return undefined;
  const out = new Uint8Array(w * h * 4);
  const yBytes = y.bytes;
  const uBytes = u.bytes;
  const vBytes = v.bytes;
  const yStride = y.stride;
  const uStride = u.stride;
  const vStride = v.stride;
  const x0 = format.cropLeft;
  const y0 = format.cropTop;
  const srcW = format.cropWidth;
  const srcH = format.cropHeight;
  const xScale = srcW / w;
  const yScale = srcH / h;
  for (let row = 0; row < h; row++) {
    const srcY = y0 + Math.min(srcH - 1, (row * yScale) | 0);
    const yOff = srcY * yStride;
    const cOff = (srcY >> 1) * uStride;
    const cOffV = (srcY >> 1) * vStride;
    let dst = row * w * 4;
    for (let col = 0; col < w; col++) {
      const srcX = x0 + Math.min(srcW - 1, (col * xScale) | 0);
      const Y = yBytes[yOff + srcX] ?? 0;
      const Cb = (uBytes[cOff + (srcX >> 1)] ?? 128) - 128;
      const Cr = (vBytes[cOffV + (srcX >> 1)] ?? 128) - 128;
      out[dst] = clamp8((Y + 1.402 * Cr + 0.5) | 0);
      out[dst + 1] = clamp8((Y - 0.344136 * Cb - 0.714136 * Cr + 0.5) | 0);
      out[dst + 2] = clamp8((Y + 1.772 * Cb + 0.5) | 0);
      out[dst + 3] = 255;
      dst += 4;
    }
  }
  return out;
}

export function rgbaIsPainted(rgba: Uint8Array | undefined): boolean {
  if (!rgba || rgba.length < 4) return false;
  for (let i = 0; i < rgba.length; i += 4) {
    if (rgba[i] || rgba[i + 1] || rgba[i + 2]) return true;
    if (rgba[i + 3] !== 0 && rgba[i + 3] !== 255) return true;
  }
  // All-black desktop is possible; alpha must still be opaque.
  return rgba[3] === 255;
}

// Flutter decodeImageFromPixels requires width*height*4 == rgba.length.
// Prefer the decoded frame size over PeerInfo (which can be 0 / default).
export function decodePixelSize(
  rgbaLength: number,
  rectW: number,
  rectH: number,
  frameW: number,
  frameH: number
): { width: number; height: number } | undefined {
  if (frameW > 0 && frameH > 0 && frameW * frameH * 4 === rgbaLength) {
    return { width: frameW, height: frameH };
  }
  if (rectW > 0 && rectH > 0 && rectW * rectH * 4 === rgbaLength) {
    return { width: rectW, height: rectH };
  }
  return undefined;
}

export function makeSolidI420(
  width: number,
  height: number,
  yVal: number,
  uVal: number,
  vVal: number
): YuvFrame {
  const yStride = width;
  const cW = Math.max(1, width >> 1);
  const cH = Math.max(1, height >> 1);
  const y = new Uint8Array(yStride * height);
  const u = new Uint8Array(cW * cH);
  const v = new Uint8Array(cW * cH);
  y.fill(yVal);
  u.fill(uVal);
  v.fill(vVal);
  return {
    format: {
      width,
      height,
      chromaWidth: cW,
      chromaHeight: cH,
      cropLeft: 0,
      cropTop: 0,
      cropWidth: width,
      cropHeight: height,
      displayWidth: width,
      displayHeight: height,
    },
    y: { bytes: y, stride: yStride },
    u: { bytes: u, stride: cW },
    v: { bytes: v, stride: cW },
  };
}
