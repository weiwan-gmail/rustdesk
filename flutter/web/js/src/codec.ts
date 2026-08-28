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

export function loadVideoDecoder(codec: "vp9" | "vp8"): Promise<VideoDecoder> {
  return new Promise((resolve, reject) => {
    if (!window.OGVLoader?.loadClass) {
      reject(new Error("OGVLoader is not available"));
      return;
    }
    simd()
      .then((isSIMD) => {
        // ogv.js 1.8.6 ships a SIMD build only for VP9, and only the non-MT
        // classes have worker proxies (see OGVLoader's class map).
        const cls =
          codec == "vp9"
            ? isSIMD
              ? "OGVDecoderVideoVP9SIMDW"
              : "OGVDecoderVideoVP9W"
            : "OGVDecoderVideoVP8W";
        window.OGVLoader.loadClass(
          cls,
          (videoCodecClass: any) => {
            videoCodecClass({ videoFormat: {} })
              .then((decoder: any) => {
                decoder.init(() => resolve(decoder));
              })
              .catch(reject);
          },
          { worker: true, threading: true }
        );
      })
      .catch(reject);
  });
}
