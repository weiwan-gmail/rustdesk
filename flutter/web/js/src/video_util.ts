// Pure helpers for the web client's VP8/VP9-only video path. Kept free of
// browser / protobuf imports so node:test can cover the login and frame
// decisions that caused the web-direct black screen.

export const MAX_PENDING_VIDEO_FRAMES = 15;

export type VideoFrameKind = "vp9" | "vp8" | "h264" | "h265" | "av1";

export type VideoFrameFields = {
  vp9s?: unknown;
  vp8s?: unknown;
  h264s?: unknown;
  h265s?: unknown;
  av1s?: unknown;
};

export type VideoFrameAction =
  | { type: "decode"; kind: "vp9" | "vp8" }
  | { type: "queue"; kind: "vp9" | "vp8" }
  | { type: "renegotiate"; kind: "h264" | "h265" | "av1" }
  | { type: "ignore" };

// Matches native LoginRequest.option.supported_decoding for a peer that can
// only paint ogv.js VP8/VP9. Prefer stays Auto (0): with H264/AV1/H265
// abilities left unset the host auto-codec is VP9 (or VP8 on low-memory).
export function webSupportedDecodingPartial(): {
  ability_vp9: number;
  ability_vp8: number;
} {
  return { ability_vp9: 1, ability_vp8: 1 };
}

export function videoFrameKind(vf: VideoFrameFields): VideoFrameKind | undefined {
  if (vf.vp9s) return "vp9";
  if (vf.vp8s) return "vp8";
  if (vf.h264s) return "h264";
  if (vf.h265s) return "h265";
  if (vf.av1s) return "av1";
  return undefined;
}

export function videoFrameAction(
  kind: VideoFrameKind | undefined,
  decoderReady: boolean
): VideoFrameAction {
  if (kind === "vp9" || kind === "vp8") {
    return decoderReady ? { type: "decode", kind } : { type: "queue", kind };
  }
  if (kind === "h264" || kind === "h265" || kind === "av1") {
    return { type: "renegotiate", kind };
  }
  return { type: "ignore" };
}

export function enqueueVideoFrame<T>(queue: T[], frame: T, max = MAX_PENDING_VIDEO_FRAMES): T[] {
  const next = queue.concat(frame);
  if (next.length <= max) return next;
  return next.slice(next.length - max);
}
