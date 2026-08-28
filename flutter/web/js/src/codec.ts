// VP8/VP9 decoding via ogv.js (wasm). The SIMD builds ship only in the
// upstream release zip, fetched by deploy/v2 fetch-codecs.sh.
import { simd } from "wasm-feature-detect";
import { ogvDecoderClassName, ogvLoaderOptions } from "./paint_util";

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
        const cls = ogvDecoderClassName(codec, isSIMD);
        const load = (name: string, onFail: (e: unknown) => void) => {
          try {
            window.OGVLoader.loadClass(
              name,
              (videoCodecClass: any) => {
                videoCodecClass({ videoFormat: {} })
                  .then((decoder: any) => {
                    decoder.init(() => resolve(decoder));
                  })
                  .catch(onFail);
              },
              ogvLoaderOptions()
            );
          } catch (e) {
            onFail(e);
          }
        };
        load(cls, (e) => {
          // SIMD wasm is optional (npm ogv has no SIMD). Fall back so
          // processFrame is not stuck with no decoder.
          if (codec == "vp9" && isSIMD) {
            load(ogvDecoderClassName("vp9", false), reject);
          } else {
            reject(e);
          }
        });
      })
      .catch(reject);
  });
}
