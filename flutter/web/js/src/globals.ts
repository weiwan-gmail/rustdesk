// Window-level bridge between the Flutter (Dart) side and the JS protocol
// stack. Dart calls window.setByName/getByName/init/isMobile; the JS side
// reports back through the window callbacks registered by Dart:
//   onInitFinished(), onGlobalEvent(json), onRgba(display, bytes),
//   onRegisteredEvent(json), dialog(type, title, text), loginDialog(),
//   closeConnection(), onFullscreenChanged(bool),
//   onLoadAbFinished(s), onLoadGroupFinished(s)
import Connection from "./connection";
import _sodium from "libsodium-wrappers";
import { loadVideoDecoder } from "./codec";
import { checkIfRetry, version } from "./gen_js_from_hbb";
import { initZstd, translate } from "./common";
import PCMPlayer from "pcm-player";
import { i420ToRgba, normalizeOgvFrame } from "./paint_util";

declare global {
  interface Window {
    curConn: Connection | undefined;
    RUSTDESK_CONFIG: any;
    YUVCanvas: any;
    setByName: (name: string, value?: any) => any;
    getByName: (name: string, arg?: any) => string;
    init: () => Promise<void>;
    isMobile: () => boolean;
    onInitFinished: () => void;
    onGlobalEvent: (message: string) => void;
    onRgba: (display: number, rgba: Uint8Array, width?: number, height?: number) => void;
    onRegisteredEvent: (message: string) => void;
  }
}

window.curConn = undefined;

window.isMobile = () => {
  return /(android|bb\d+|meego).+mobile|avantgo|bada\/|blackberry|blazer|compal|elaine|fennec|hiptop|iemobile|ip(hone|od)|ipad|iris|kindle|Android|Silk|lge |maemo|midp|mmp|netfront|opera m(ob|in)i|palm( os)?|phone|p(ixi|re)\/|plucker|pocket|psp|series(4|6)0|symbian|treo|up\.(browser|link)|vodafone|wap|windows (ce|phone)|xda|xiino/i.test(
    navigator.userAgent
  );
};

export function isDesktop() {
  return !window.isMobile();
}

export function msgbox(type: string, title: string, text: string, link?: string) {
  if (!type || (type == "error" && !text)) return;
  const hasRetry = checkIfRetry(type, title, text, false) ? "true" : "";
  pushEventRaw(
    "msgbox",
    { name: "msgbox", type, title, text, link: link ?? "", hasRetry },
    true
  );
}

function jsonfyForDart(payload: any): any {
  const tmp: any = {};
  for (const [key, value] of Object.entries(payload)) {
    if (!key) continue;
    if (value instanceof String || typeof value == "string") {
      tmp[key] = value;
    } else if (value instanceof Uint8Array) {
      tmp[key] = "[" + value.toString() + "]";
    } else {
      tmp[key] = JSON.stringify(value);
    }
  }
  return tmp;
}

function pushEventRaw(name: string, payload: any, alreadyJsonfied = false) {
  const data = alreadyJsonfied ? payload : jsonfyForDart(payload);
  data.name = name;
  window.onGlobalEvent(JSON.stringify(data));
}

export function pushEvent(name: string, payload: any) {
  pushEventRaw(name, payload);
}

// ========================== video begin ==========================
// Do not go through YUVCanvas.attach + getContext("webgl") + readPixels.
// attach() already creates a WebGL context with preserveDrawingBuffer;
// a second getContext("webgl") without those attributes is null on Chrome,
// and the old draw() then took neither the worker nor the GL branch.
// yuv.js also needs yuv.wasm, which fetch-codecs.sh does not ship.
// Software I420→RGBA is the path that always produces a Flutter Image.

export function draw(display: number, frame: any) {
  if (!window.onRgba) return;
  try {
    const yuv = normalizeOgvFrame(frame);
    if (!yuv) return;
    const rgba = i420ToRgba(yuv);
    if (!rgba) return;
    // Copy: Dart decodeImageFromPixels can detach the JS ArrayBuffer.
    window.onRgba(
      display,
      new Uint8Array(rgba),
      yuv.format.displayWidth,
      yuv.format.displayHeight
    );
  } catch (e) {
    console.error("Failed to draw video frame: " + e);
  }
}
// ========================== video end ============================

// ========================== audio begin ==========================
let opusWorker: Worker | undefined;
let pcmPlayer: any;

export function initAudio(channels: number, sampleRate: number) {
  try {
    if (!opusWorker) {
      opusWorker = new Worker("./libopus.js");
      opusWorker.onmessage = (e) => {
        pcmPlayer?.feed(e.data);
      };
    }
    pcmPlayer = new PCMPlayer({
      channels,
      sampleRate,
      flushingTime: 2000,
    } as any);
    opusWorker.postMessage({ channels, sampleRate });
  } catch (e) {
    console.error("Failed to init audio: " + e);
  }
}

export function playAudio(packet: Uint8Array) {
  try {
    opusWorker?.postMessage(packet, [packet.buffer]);
  } catch (e) {
    console.error("Failed to play audio: " + e);
  }
}
// ========================== audio end ============================

// ========================== crypto begin ==========================
let sodium: typeof _sodium | undefined;

async function readySodium() {
  if (!sodium) {
    await _sodium.ready;
    sodium = _sodium;
  }
  return sodium;
}

export async function verify(signed: Uint8Array, pk: string | Uint8Array) {
  const s = await readySodium();
  if (typeof pk == "string") {
    pk = s.from_base64(pk, s.base64_variants.ORIGINAL);
  }
  return s.crypto_sign_open(signed, pk);
}

export function genBoxKeyPair(): [Uint8Array, Uint8Array] {
  const pair = sodium!.crypto_box_keypair();
  return [pair.privateKey, pair.publicKey];
}

export function genSecretKey() {
  return sodium!.crypto_secretbox_keygen();
}

export function seal(unsigned: Uint8Array, theirPk: Uint8Array, ourSk: Uint8Array) {
  const nonce = Uint8Array.from(Array(24).fill(0));
  return sodium!.crypto_box_easy(unsigned, nonce, theirPk, ourSk);
}

function makeOnce(value: number) {
  const byteArray = Array(24).fill(0);
  for (let index = 0; index < byteArray.length && value > 0; index++) {
    const byte = value & 0xff;
    byteArray[index] = byte;
    value = (value - byte) / 256;
  }
  return Uint8Array.from(byteArray);
}

export function encrypt(unsigned: Uint8Array, nonce: number, key: Uint8Array) {
  return sodium!.crypto_secretbox_easy(unsigned, makeOnce(nonce), key);
}

export function decrypt(signed: Uint8Array, nonce: number, key: Uint8Array) {
  return sodium!.crypto_secretbox_open_easy(signed, makeOnce(nonce), key);
}

// Persistent random hwid for 2FA / login identification.
export function getHwid(): Uint8Array {
  let hwid = localStorage.getItem("hwid");
  if (!hwid) {
    const bytes = new Uint8Array(32);
    crypto.getRandomValues(bytes);
    hwid = Array.from(bytes).join(",");
    localStorage.setItem("hwid", hwid);
  }
  return Uint8Array.from(hwid.split(",").map((x) => parseInt(x)));
}
// ========================== crypto end ============================

// ========================== session begin ==========================
export function setConn(conn: Connection | undefined) {
  window.curConn = conn;
}

export function getConn() {
  return window.curConn;
}

export async function startConn(id: string) {
  window.setByName("remote_id", id);
  await window.curConn?.start(id);
}

export function close() {
  getConn()?.close();
  setConn(undefined);
}

export function newConn() {
  window.curConn?.close();
  const conn = new Connection();
  setConn(conn);
  return conn;
}

function sessionAdd(value: string): string {
  try {
    const data = JSON.parse(value);
    window.curConn?.close();
    const conn = new Connection();
    setConn(conn);
    if (data["password"]) {
      try {
        conn._password = Uint8Array.from(JSON.parse("[" + data["password"] + "]"));
      } catch (e) {
        console.error("Failed to parse password, " + e);
      }
    }
    return "";
  } catch (e: any) {
    return e.message;
  }
}

function sessionStart(value: string) {
  try {
    const conn = getConn();
    if (!conn) return;
    const data = JSON.parse(value);
    if (data["id"]) {
      startConn(data["id"]);
    } else {
      msgbox("error", "Error", "No id found in session data " + value, "");
    }
  } catch (e: any) {
    msgbox("error", "Error", e.message, "");
  }
}
// ========================== session end ============================

// ========================== options begin ==========================
export function getPeers(): any {
  return getJsonObj("peers");
}

export function getJsonObj(key: string): any {
  try {
    return JSON.parse(localStorage.getItem(key) || "") || {};
  } catch (e) {
    return {};
  }
}

function setUserDefaultOption(value: string) {
  try {
    const obj = JSON.parse(value);
    const userDefaultOptions = getJsonObj("user-default-options");
    userDefaultOptions[obj.name] = obj.value;
    localStorage.setItem("user-default-options", JSON.stringify(userDefaultOptions));
  } catch (e) {
    console.error("Failed to set user default options: " + e);
  }
}

export function getUserDefaultOption(value: string): string {
  const defaultOptions: Record<string, string> = {
    view_style: "original",
    scroll_style: "scrollauto",
    image_quality: "balanced",
    "codec-preference": "auto",
    custom_image_quality: "50",
    "custom-fps": "30",
  };
  try {
    const userDefaultOptions = getJsonObj("user-default-options");
    return userDefaultOptions[value] || defaultOptions[value] || "";
  } catch (e) {
    return defaultOptions[value] || "";
  }
}

function getPeerOption(value: string): any {
  try {
    const obj = JSON.parse(value);
    const options = getPeers()[obj.id] || {};
    return options[obj.name] ?? getUserDefaultOption(obj.name);
  } catch (e) {
    console.error('Failed to get peer option: "' + value + '", ' + e);
    return "";
  }
}

function setPeerOption(param: string) {
  try {
    const obj = JSON.parse(param);
    const peers = getPeers();
    const options = peers[obj.id] || {};
    if (obj.value == undefined || obj.value === "") {
      delete options[obj.name];
    } else {
      options[obj.name] = obj.value;
    }
    options["tm"] = new Date().getTime();
    peers[obj.id] = options;
    localStorage.setItem("peers", JSON.stringify(peers));
  } catch (e) {
    console.error('Failed to set peer option: "' + param + '", ' + e);
  }
}
// ========================== options end ============================

// ========================== peers begin ==========================
function getRecentPeers(): any[] {
  const peers: any[] = [];
  for (const [id, value] of Object.entries(getPeers() as any)) {
    if (!id) continue;
    const tm = (value as any)["tm"];
    const info = (value as any)["info"] || {};
    const cardInfo = {
      id: id,
      username: info["username"] || "",
      hostname: info["hostname"] || "",
      platform: info["platform"] || "",
      alias: (value as any).alias || "",
    };
    if (!tm) continue;
    peers.push([tm, id, cardInfo]);
  }
  return peers.sort().reverse().map((x) => x[2]);
}

function loadRecentPeers() {
  const peersRecent = getRecentPeers();
  window.onRegisteredEvent(
    JSON.stringify({ name: "load_recent_peers", peers: JSON.stringify(peersRecent) })
  );
}

function loadFavPeers() {
  try {
    const favs = JSON.parse(localStorage.getItem("fav") ?? "[]");
    const peersFav = getRecentPeers().filter((x) => favs.includes(x.id));
    window.onRegisteredEvent(
      JSON.stringify({ name: "load_fav_peers", peers: JSON.stringify(peersFav) })
    );
  } catch (e) {
    console.error("Failed to load fav peers: " + e);
  }
}
// ========================== peers end ============================

// ========================== server begin ==========================
function increasePort(host: string, offset: number): string {
  function isIPv6(str: string) {
    return /^([0-9a-fA-F]{0,4}:){1,7}[0-9a-fA-F]{0,4}$/.test(str);
  }
  if (isIPv6(host)) {
    if (host.startsWith("[")) {
      const tmp = host.split("]:");
      if (tmp.length === 2) {
        const port = parseInt(tmp[1]) || 0;
        if (port > 0) return `${tmp[0]}]:${port + offset}`;
      }
    }
  } else if (host.includes(":")) {
    const tmp = host.split(":");
    if (tmp.length === 2) {
      const port = parseInt(tmp[1]) || 0;
      if (port > 0) return `${tmp[0]}:${port + offset}`;
    }
  }
  return host;
}

function getApiServer(): string {
  const apiServer = localStorage.getItem("api-server");
  if (apiServer) return apiServer;
  const customRendezvousServer = localStorage.getItem("custom-rendezvous-server");
  if (customRendezvousServer) {
    const s = increasePort(customRendezvousServer, -2);
    if (s == customRendezvousServer) {
      return `http://${s}:21114`;
    }
    return `http://${s}`;
  }
  return "https://admin.rustdesk.com";
}

function getAuditServer(typ: string): string {
  if (!localStorage.getItem("access_token")) return "";
  const apiServer = getApiServer();
  if (!apiServer || apiServer.includes("rustdesk.com")) return "";
  return apiServer + "/api/audit/" + typ;
}
// ========================== server end ============================

// Dup to the function in hbb_common, lib.rs
export function getVersionNumber(v: string): number {
  try {
    const versions = v.split("-");
    let n = 0;
    if (versions.length > 0) {
      let last = 0;
      for (const x of versions[0].split(".")) {
        last = parseInt(x) || 0;
        n = n * 1000 + last;
      }
      n -= last;
      n += last * 10;
    }
    if (versions.length > 1) {
      n += parseInt(versions[1]) || 0;
    }
    return n;
  } catch (e: any) {
    console.error('Failed to parse version number: "' + v + '" ' + e.message);
    return 0;
  }
}

// Set the cursor for the flutter-view element. The Dart side sends either a
// JSON object ({url, hotx, hoty}) or a plain CSS cursor value ("auto").
function setCustomCursor(value: string) {
  let cursor = value || "auto";
  try {
    const obj = JSON.parse(value);
    cursor = `url(${obj.url}) ${obj.hotx} ${obj.hoty}, auto`;
  } catch (e) {
    // plain CSS cursor value
  }
  const body = document.body;
  for (let i = 0; i < body.children.length; i++) {
    const child = body.children[i] as HTMLElement;
    if (child.tagName == "FLUTTER-VIEW") {
      child.style.cursor = cursor;
    }
  }
}

export function copyToClipboard(text: string) {
  if (navigator.clipboard?.writeText) {
    navigator.clipboard.writeText(text).catch((e) => {
      console.warn("Copy to clipboard failed.", e);
    });
    return;
  }
  const textarea = document.createElement("textarea");
  textarea.textContent = text;
  textarea.style.position = "fixed";
  document.body.appendChild(textarea);
  textarea.select();
  try {
    document.execCommand("copy");
  } catch (ex) {
    console.warn("Copy to clipboard failed.", ex);
  } finally {
    document.body.removeChild(textarea);
  }
}

const localEnv: Record<string, string> = {};

window.setByName = (name: string, value?: any): any => {
  const conn = window.curConn;
  switch (name) {
    case "remote_id":
      localStorage.setItem("remote-id", value);
      break;
    case "connect":
      newConn();
      startConn(value);
      break;
    case "login": {
      const v = JSON.parse(value);
      conn?.setRemember(v.remember);
      conn?.login({
        os_login: { username: v.os_username, password: v.os_password },
        password: v.password,
      });
      break;
    }
    case "close":
    case "session_close":
      close();
      break;
    case "refresh":
      conn?.refresh();
      break;
    case "reconnect":
      conn?.reconnect();
      break;
    case "toggle_option":
    case "option:toggle":
      conn?.toggleOption(value);
      break;
    case "toggle_privacy_mode":
      conn?.togglePrivacyMode(value);
      break;
    case "toggle_virtual_display":
      conn?.toggleVirtualDisplay(value);
      break;
    case "image_quality":
      conn?.setImageQuality(value);
      break;
    case "custom_image_quality":
      conn?.setCustomImageQuality(value);
      break;
    case "custom-fps":
      conn?.setCustomFps(value);
      break;
    case "lock_screen":
      conn?.lockScreen();
      break;
    case "ctrl_alt_del":
      conn?.ctrlAltDel();
      break;
    case "switch_display":
      conn?.switchDisplay(value);
      break;
    case "change_resolution":
      conn?.changeResolution(value);
      break;
    case "selected_sid":
      conn?.sendSelectedSessionId(value);
      break;
    case "remove_peer": {
      const peers = getPeers();
      delete peers[value];
      localStorage.setItem("peers", JSON.stringify(peers));
      break;
    }
    case "input_key": {
      const v = JSON.parse(value);
      conn?.inputKey(
        v.name,
        v.down == "true",
        v.press == "true",
        v.alt == "true",
        v.ctrl == "true",
        v.shift == "true",
        v.command == "true"
      );
      break;
    }
    case "input_string":
      conn?.inputString(value);
      break;
    case "send_mouse": {
      if (!conn) return;
      let mask = 0;
      const v = JSON.parse(value);
      switch (v.type) {
        case "down":
          mask = 1;
          break;
        case "up":
          mask = 2;
          break;
        case "wheel":
          mask = 3;
          break;
      }
      switch (v.buttons) {
        case "left":
          mask |= 1 << 3;
          break;
        case "right":
          mask |= 2 << 3;
          break;
        case "wheel":
          mask |= 4 << 3;
          break;
      }
      conn.inputMouse(
        mask,
        parseInt(v.x || "0"),
        parseInt(v.y || "0"),
        v.alt == "true",
        v.ctrl == "true",
        v.shift == "true",
        v.command == "true"
      );
      break;
    }
    case "send_2fa": {
      try {
        const v = JSON.parse(value);
        conn?.send2fa(v.code ?? value);
      } catch (e) {
        conn?.send2fa(value);
      }
      break;
    }
    case "option": {
      const v = JSON.parse(value);
      localStorage.setItem(v.name, v.value);
      break;
    }
    case "options": {
      const v = JSON.parse(value);
      for (const [k, val] of Object.entries(v)) {
        localStorage.setItem(k, String(val));
      }
      break;
    }
    case "option:local":
    case "option:flutter:local":
    case "option:flutter:peer": {
      const v = JSON.parse(value);
      localStorage.setItem(name + ":" + v.name, v.value);
      break;
    }
    case "option:user:default":
      setUserDefaultOption(value);
      break;
    case "option:session": {
      const v = JSON.parse(value);
      conn?.setOption(v.name, v.value);
      break;
    }
    case "option:peer":
      setPeerOption(value);
      break;
    case "input_os_password":
      conn?.inputOsPassword(value);
      break;
    case "session_add_sync":
      return sessionAdd(value);
    case "session_start":
      sessionStart(value);
      break;
    case "elevate_direct":
      conn?.elevateDirect();
      break;
    case "elevate_with_logon":
      conn?.elevateWithLogon(value);
      break;
    case "forget":
      conn?.setRemember(false);
      break;
    case "restart":
      conn?.restart();
      break;
    case "fav":
      localStorage.setItem("fav", value);
      break;
    case "change_prefer_codec":
      conn?.changePreferCodec();
      break;
    case "cursor":
      setCustomCursor(value);
      break;
    case "envvar": {
      const v = JSON.parse(value);
      localEnv[v.name] = v.value ?? "";
      break;
    }
    case "enter_or_leave":
      // Pointer lock / focus bookkeeping is handled by the Flutter side.
      break;
    case "flutter_key_event":
      // Raw flutter key events are only used with input source 2, which the
      // web client does not enable (mainGetInputSource defaults to source 1).
      break;
    case "audit_guid":
      localStorage.setItem("audit_guid", value);
      break;
    case "send_note":
      // Audit note upload requires a pro server; no-op on web for now.
      break;
    case "save_ab":
      localStorage.setItem("ab", value);
      break;
    case "clear_ab":
      localStorage.removeItem("ab");
      break;
    case "load_ab":
      (window as any).onLoadAbFinished?.(localStorage.getItem("ab") || "");
      break;
    case "save_group":
      localStorage.setItem("group", value);
      break;
    case "clear_group":
      localStorage.removeItem("group");
      break;
    case "load_group":
      (window as any).onLoadGroupFinished?.(localStorage.getItem("group") || "");
      break;
    case "account_auth":
    case "account_auth_cancel":
      // Account login (OIDC) is not supported on the web client yet.
      break;
    default:
      break;
  }
};

window.getByName = (name: string, arg?: any): string => {
  const v = _getByName(name, arg);
  if (typeof v == "string" || v instanceof String) return v as string;
  if (v == undefined || v == null) return "";
  return JSON.stringify(v);
};

function _getByName(name: string, arg: any): any {
  const conn = window.curConn;
  switch (name) {
    case "remote_id":
      return localStorage.getItem("remote-id");
    case "remember":
      return conn?.getRemember() ?? false;
    case "option":
      return localStorage.getItem(arg);
    case "options": {
      const keys = ["custom-rendezvous-server", "relay-server", "api-server", "key"];
      const obj: Record<string, string> = {};
      keys.forEach((key) => {
        const v = localStorage.getItem(key);
        if (v) obj[key] = v;
      });
      return JSON.stringify(obj);
    }
    case "option:local":
    case "option:flutter:local":
    case "option:flutter:peer":
      return localStorage.getItem(name + ":" + arg);
    case "image_quality":
      return conn?.getImageQuality() ?? getUserDefaultOption("image_quality");
    case "translate": {
      const v = JSON.parse(arg);
      return translate(v.locale, v.text);
    }
    case "option:user:default":
      return getUserDefaultOption(arg);
    case "option:session":
      if (conn) {
        return conn.getOption(arg);
      }
      return getUserDefaultOption(arg);
    case "option:peer":
      return getPeerOption(arg);
    case "option:toggle":
      return conn?.getToggleOption(arg) ?? false;
    case "get_conn_status":
      if (conn) {
        return conn.getStatus();
      }
      return JSON.stringify({ status_num: 0 });
    case "app-name":
      return (window.RUSTDESK_CONFIG || {}).appName || "RustDesk";
    case "version":
      return version;
    case "build_date":
      return "";
    case "load_recent_peers":
      loadRecentPeers();
      return "";
    case "load_fav_peers":
      loadFavPeers();
      return "";
    case "fav":
      return localStorage.getItem("fav") ?? "[]";
    case "load_recent_peers_sync":
      return JSON.stringify({ peers: JSON.stringify(getRecentPeers()) });
    case "api_server":
      return getApiServer();
    case "is_using_public_server":
      return !localStorage.getItem("custom-rendezvous-server");
    case "get_version_number":
      return getVersionNumber(arg);
    case "audit_server":
      return getAuditServer(arg);
    case "audit_guid":
      return localStorage.getItem("audit_guid") || "";
    case "last_audit_note":
      return "";
    case "alternative_codecs":
      return JSON.stringify({ vp8: true, av1: false, h264: false, h265: false });
    case "screen_info": {
      const scr = window.screen as any;
      return JSON.stringify({
        frame: {
          l: window.screenX,
          t: window.screenY,
          r: window.screenX + window.innerWidth,
          b: window.screenY + window.innerHeight,
        },
        visibleFrame: {
          l: scr.availLeft,
          t: scr.availTop,
          r: scr.availLeft + scr.availWidth,
          b: scr.availTop + scr.availHeight,
        },
        scaleFactor: window.devicePixelRatio,
      });
    }
    case "main_display":
      return JSON.stringify({
        w: window.screen.availWidth,
        h: window.screen.availHeight,
        scaleFactor: window.devicePixelRatio,
      });
    case "local_os":
      return detectLocalOs();
    case "my_id":
      return localStorage.getItem("my_id") || "";
    case "my_name":
      return localStorage.getItem("my_name") || "web";
    case "uuid":
      return getOrCreateUuid();
    case "langs":
      return JSON.stringify([
        { name: "English", code: "en" },
        { name: "中文", code: "cn" },
      ]);
    case "envvar":
      return localEnv[arg] || "";
    case "peer_has_password": {
      const options = getPeers()[arg] || {};
      return (options["password"] ?? "") !== "";
    }
    case "peer_exists":
      return !!getPeers()[arg];
    case "platform":
      return conn?._peerInfo?.platform || "";
    case "conn_session_id":
      return "";
    case "enable_trusted_devices":
      return "Y";
    case "resolve_avatar_url":
      return arg;
    case "account_auth_result":
      return "";
  }
  return "";
}

function detectLocalOs(): string {
  const ua = navigator.userAgent;
  if (ua.indexOf("Win") >= 0) return "Windows";
  if (ua.indexOf("Mac") >= 0) return "Mac OS";
  if (ua.indexOf("Linux") >= 0) return "Linux";
  if (ua.indexOf("Android") >= 0) return "Android";
  return "";
}

function getOrCreateUuid(): string {
  let uuid = localStorage.getItem("uuid");
  if (!uuid) {
    uuid = crypto.randomUUID();
    localStorage.setItem("uuid", uuid);
  }
  return uuid;
}

window.init = async () => {
  try {
    await readySodium();
  } catch (e) {
    console.error("Failed to init sodium: " + e);
  }
  try {
    void loadVideoDecoder("vp9").catch((e) => {
      console.error("Failed to preload vp9 decoder: " + e);
    });
  } catch (e) {
    console.error("Failed to preload vp9 decoder: " + e);
  }
  try {
    await initZstd();
  } catch (e) {
    console.error("Failed to init zstd: " + e);
  }
  console.log("init done");
  window.onInitFinished();
};
