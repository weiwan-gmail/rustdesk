import * as zstd from "zstddec";
import { KeyEvent, controlKeyFromJSON, ControlKey } from "./message";
import { KEY_MAP, LANGS } from "./gen_js_from_hbb";

let decompressor: zstd.ZSTDDecoder | undefined;

export async function initZstd() {
  const dec = new zstd.ZSTDDecoder();
  await dec.init();
  decompressor = dec;
}

export async function decompress(compressed: Uint8Array): Promise<Uint8Array | undefined> {
  const MAX = 1024 * 1024 * 64;
  const MIN = 1024 * 1024;
  let n = 30 * compressed.length;
  if (n > MAX) n = MAX;
  if (n < MIN) n = MIN;
  try {
    if (!decompressor) await initZstd();
    return decompressor!.decode(compressed, n);
  } catch (e) {
    console.error("decompress failed: " + e);
    return undefined;
  }
}

const LANG = queryLang();

export function translate(locale: string, text: string): string {
  const lang = LANG || locale.substring(locale.length - 2).toLowerCase();
  const en = (LANGS as any).en as any;
  let dict = (LANGS as any)[lang];
  if (!dict) dict = en;
  let res = dict ? dict[text] : undefined;
  if (!res && lang != "en" && en) res = en[text];
  return res || text;
}

const zCode = "z".charCodeAt(0);
const aCode = "a".charCodeAt(0);

export function mapKey(name: string, isDesktop: Boolean): KeyEvent | undefined {
  const tmp = KEY_MAP[name] || name;
  if (tmp.length == 1) {
    const chr = tmp.charCodeAt(0);
    if (!isDesktop && (chr > zCode || chr < aCode)) {
      return KeyEvent.fromPartial({ unicode: chr });
    }
    return KeyEvent.fromPartial({ chr });
  }
  const control_key = controlKeyFromJSON(tmp);
  if (control_key == ControlKey.UNRECOGNIZED) {
    console.error("Unknown control key " + tmp);
    return undefined;
  }
  return KeyEvent.fromPartial({ control_key });
}

export async function sleep(ms: number) {
  await new Promise((r) => setTimeout(r, ms));
}

function queryLang(): string {
  try {
    return new URLSearchParams(window.location.search).get("lang") || "";
  } catch (e) {
    return "";
  }
}
