import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  decodePixelSize,
  i420ToRgba,
  legacySelectDrawBackend,
  makeSolidI420,
  normalizeOgvFrame,
  ogvDecoderClassName,
  ogvLoaderOptions,
  rgbaIsPainted,
  selectDrawBackend,
  shouldPaintDecodedFrame,
} from "./paint_util.ts";

describe("legacySelectDrawBackend (pre-fix globals.draw)", () => {
  it("is a no-op when WebGL was chosen but getContext returned null", () => {
    // WebGLFrameSink.isAvailable() true → no yuvWorker. attach() already
    // created a context with preserveDrawingBuffer; a second
    // getContext("webgl") without those attrs is null. Old draw() then
    // takes neither branch and the canvas stays black.
    assert.equal(
      legacySelectDrawBackend({ yuvWorker: undefined, gl: null }),
      "none"
    );
  });
});

describe("selectDrawBackend", () => {
  it("falls back to software instead of dropping the frame", () => {
    assert.equal(selectDrawBackend({ yuvWorker: undefined, gl: null }), "software");
    assert.equal(selectDrawBackend({ yuvWorker: {}, gl: null }), "worker");
    assert.equal(selectDrawBackend({ yuvWorker: undefined, gl: {} }), "webgl");
  });
});

describe("ogv decoder load", () => {
  it("uses the 1.8.6 worker class names (SIMD only for VP9)", () => {
    assert.equal(ogvDecoderClassName("vp9", true), "OGVDecoderVideoVP9SIMDW");
    assert.equal(ogvDecoderClassName("vp9", false), "OGVDecoderVideoVP9W");
    assert.equal(ogvDecoderClassName("vp8", true), "OGVDecoderVideoVP8W");
  });
  it("does not request threading (no COOP/COEP on web-direct)", () => {
    const o = ogvLoaderOptions();
    assert.equal(o.worker, true);
    assert.equal("threading" in o, false);
  });
});

describe("shouldPaintDecodedFrame", () => {
  it("paints when processFrame omits ok but frameBuffer is set", () => {
    assert.equal(shouldPaintDecodedFrame(undefined, { y: {} }), true);
    assert.equal(shouldPaintDecodedFrame(true, { y: {} }), true);
    assert.equal(shouldPaintDecodedFrame(false, { y: {} }), false);
    assert.equal(shouldPaintDecodedFrame(true, undefined), false);
  });
});

describe("normalizeOgvFrame", () => {
  it("fills display/chroma from coded size when the worker dropped them", () => {
    const y = new Uint8Array(8);
    const u = new Uint8Array(2);
    const v = new Uint8Array(2);
    const n = normalizeOgvFrame({
      format: { width: 4, height: 2 },
      y: { bytes: y, stride: 4 },
      u: { bytes: u, stride: 2 },
      v: { bytes: v, stride: 2 },
    });
    assert.ok(n);
    assert.equal(n.format.displayWidth, 4);
    assert.equal(n.format.displayHeight, 2);
    assert.equal(n.format.chromaWidth, 2);
    assert.equal(n.format.chromaHeight, 1);
    assert.equal(n.format.cropWidth, 4);
  });
  it("accepts raw Uint8Array planes (detached-then-cloned worker shape)", () => {
    const n = normalizeOgvFrame({
      format: { width: 2, height: 2, displayWidth: 2, displayHeight: 2 },
      y: new Uint8Array(4),
      u: new Uint8Array(1),
      v: new Uint8Array(1),
    });
    assert.ok(n);
    assert.equal(n.y.stride, 2);
    assert.equal(n.u.stride, 1);
  });
  it("rejects a frame with no size (yuv-canvas would leave canvas 0x0)", () => {
    assert.equal(normalizeOgvFrame({ y: { bytes: new Uint8Array(1), stride: 1 } }), undefined);
  });
});

describe("i420ToRgba", () => {
  it("turns a white I420 desktop into opaque non-black RGBA", () => {
    const frame = makeSolidI420(4, 2, 255, 128, 128);
    const rgba = i420ToRgba(frame);
    assert.ok(rgba);
    assert.equal(rgba.length, 4 * 2 * 4);
    assert.equal(rgbaIsPainted(rgba), true);
    // White YUV → near-white RGB, opaque.
    assert.ok(rgba[0] > 240);
    assert.ok(rgba[1] > 240);
    assert.ok(rgba[2] > 240);
    assert.equal(rgba[3], 255);
  });
  it("keeps a black desktop opaque so Flutter does not drop the buffer", () => {
    const rgba = i420ToRgba(makeSolidI420(2, 2, 0, 128, 128));
    assert.ok(rgba);
    assert.equal(rgba[0], 0);
    assert.equal(rgba[3], 255);
    assert.equal(rgbaIsPainted(rgba), true);
  });
});

describe("decodePixelSize", () => {
  it("proves the Flutter getDisplayRect mismatch: 1080x720 vs real frame", () => {
    const real = 1920 * 1080 * 4;
    // Old path: decodeImageFromPixels(rgba, 1080, 720) → instantiateCodec fail.
    assert.equal(1080 * 720 * 4 === real, false);
    assert.equal(decodePixelSize(real, 1080, 720, 0, 0), undefined);
  });
  it("uses the decoded frame size when PeerInfo rect is default or zero", () => {
    const real = 1920 * 1080 * 4;
    assert.deepEqual(decodePixelSize(real, 1080, 720, 1920, 1080), {
      width: 1920,
      height: 1080,
    });
    assert.deepEqual(decodePixelSize(16, 0, 0, 2, 2), { width: 2, height: 2 });
  });
});
