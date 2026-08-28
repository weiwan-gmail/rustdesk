import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  enqueueVideoFrame,
  MAX_PENDING_VIDEO_FRAMES,
  shouldAutoSelectWindowsSession,
  sidToStartVideo,
  videoFrameAction,
  videoFrameKind,
  webSupportedDecodingPartial,
  windowsSessionsForPicker,
} from "./video_util.ts";

describe("webSupportedDecodingPartial", () => {
  it("advertises only VP8/VP9 so the host will not pick H264/AV1", () => {
    const d = webSupportedDecodingPartial();
    assert.equal(d.ability_vp9, 1);
    assert.equal(d.ability_vp8, 1);
    assert.equal("ability_h264" in d, false);
    assert.equal("ability_h265" in d, false);
    assert.equal("ability_av1" in d, false);
  });
});

describe("videoFrameKind", () => {
  it("prefers VP9 over VP8 when both are set", () => {
    assert.equal(videoFrameKind({ vp9s: {}, vp8s: {} }), "vp9");
  });
  it("recognizes VP8 and unsupported codecs", () => {
    assert.equal(videoFrameKind({ vp8s: {} }), "vp8");
    assert.equal(videoFrameKind({ h264s: {} }), "h264");
    assert.equal(videoFrameKind({ h265s: {} }), "h265");
    assert.equal(videoFrameKind({ av1s: {} }), "av1");
    assert.equal(videoFrameKind({}), undefined);
  });
});

describe("videoFrameAction", () => {
  it("decodes VP8/VP9 when the decoder exists", () => {
    assert.deepEqual(videoFrameAction("vp9", true), { type: "decode", kind: "vp9" });
    assert.deepEqual(videoFrameAction("vp8", true), { type: "decode", kind: "vp8" });
  });
  it("queues VP8/VP9 until the decoder is ready (and caller still acks)", () => {
    assert.deepEqual(videoFrameAction("vp9", false), { type: "queue", kind: "vp9" });
    assert.deepEqual(videoFrameAction("vp8", false), { type: "queue", kind: "vp8" });
  });
  it("renegotiates H264/AV1/H265 instead of stub-decoding", () => {
    assert.deepEqual(videoFrameAction("h264", true), { type: "renegotiate", kind: "h264" });
    assert.deepEqual(videoFrameAction("av1", false), { type: "renegotiate", kind: "av1" });
    assert.deepEqual(videoFrameAction("h265", true), { type: "renegotiate", kind: "h265" });
  });
  it("ignores empty frames", () => {
    assert.deepEqual(videoFrameAction(undefined, true), { type: "ignore" });
  });
});

describe("windowsSessionsForPicker", () => {
  it("returns undefined when the host did not send sessions", () => {
    assert.equal(windowsSessionsForPicker(undefined), undefined);
    assert.equal(windowsSessionsForPicker({ sessions: [] }), undefined);
  });
  it("flattens proto sessions to the native Flutter picker payload", () => {
    const p = windowsSessionsForPicker({
      current_sid: 2,
      sessions: [
        { sid: 1, name: "Console: user" },
        { sid: 2, name: "RDP: user (running)" },
      ],
    });
    assert.deepEqual(p, {
      currentSid: 2,
      sessions: [
        { sid: "1", name: "Console: user" },
        { sid: "2", name: "RDP: user (running)" },
      ],
    });
  });
});

describe("shouldAutoSelectWindowsSession", () => {
  it("auto-sends only when the remembered sid is the host current_sid", () => {
    assert.equal(shouldAutoSelectWindowsSession(2, 2), true);
    assert.equal(shouldAutoSelectWindowsSession(2, 1), false);
    assert.equal(shouldAutoSelectWindowsSession(2, undefined), false);
  });
});

describe("sidToStartVideo", () => {
  const picker = {
    currentSid: 2,
    sessions: [
      { sid: "1", name: "Console" },
      { sid: "2", name: "RDP" },
    ],
  };
  it("sends current_sid when nothing is remembered so video can start", () => {
    assert.equal(sidToStartVideo(picker, undefined), "2");
  });
  it("keeps a remembered sid that is still in the list", () => {
    assert.equal(sidToStartVideo(picker, 1), "1");
  });
  it("falls back to current_sid if the remembered sid disappeared", () => {
    assert.equal(sidToStartVideo(picker, 9), "2");
  });
});

describe("enqueueVideoFrame", () => {
  it("keeps the newest frames when over cap so a late keyframe is not dropped forever", () => {
    const q = enqueueVideoFrame(
      Array.from({ length: MAX_PENDING_VIDEO_FRAMES }, (_, i) => i),
      99
    );
    assert.equal(q.length, MAX_PENDING_VIDEO_FRAMES);
    assert.equal(q[q.length - 1], 99);
    assert.equal(q[0], 1);
  });
});
