// VP8/VP9 decoding via ogv.js (wasm). The SIMD builds ship only in the
// upstream release zip, fetched by deploy/v2 fetch-codecs.sh.
import { simd } from "wasm-feature-detect";

declare global {
  interface Window {
    OGVLoader: any;
  }
}

export type VideoDecoder = {
  processFrame(data: ArrayBuffer, callback: (ok: boolean) => void): void;
  frameBuffer: any;
  close(): void;
};

export async function loadVideoDecoder(
  codec: "vp9" | "vp8",
  callback: (decoder: VideoDecoder) => void
) {
  const isSIMD = await simd();
  // ogv.js 1.8.6 ships a SIMD build only for VP9, and only the non-MT classes
  // have worker proxies (see OGVLoader's class map).
  const cls =
    codec == "vp9"
      ? isSIMD
        ? "OGVDecoderVideoVP9SIMDW"
        : "OGVDecoderVideoVP9W"
      : "OGVDecoderVideoVP8W";
  window.OGVLoader.loadClass(
    cls,
    (videoCodecClass: any) => {
      videoCodecClass({ videoFormat: {} }).then((decoder: any) => {
        decoder.init(() => callback(decoder));
      });
    },
    { worker: true, threading: true }
  );
}
