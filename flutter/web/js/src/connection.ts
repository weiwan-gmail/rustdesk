import Websock from "./websock";
import * as message from "./message.js";
import * as rendezvous from "./rendezvous.js";
import { loadVideoDecoder, VideoDecoder } from "./codec";
import * as sha256 from "fast-sha256";
import * as globals from "./globals";
import * as consts from "./consts";
import { decompress, mapKey, sleep } from "./common";
import { version } from "./gen_js_from_hbb";
import {
  enqueueVideoFrame,
  videoFrameAction,
  videoFrameKind,
  webSupportedDecodingPartial,
} from "./video_util";

export const PORT = 21116;
// Default direct-access port of the controlled client (RENDEZVOUS_PORT + 2).
export const DIRECT_PORT = 21118;
// Deployment-time configuration, served as config.js next to index.html.
// window.RUSTDESK_CONFIG = { server: "host[:port]", wsIdPath: "/ws/id", wsRelayPath: "/ws/relay", direct?: true, directPath: "/direct" }
const CONF: any = (window as any).RUSTDESK_CONFIG || {};

function wsSchema(): string {
  return location.protocol === "https:" ? "wss://" : "ws://";
}

// An IP literal (v4 or v6) with an optional port triggers direct IP access,
// mirroring the native client's is_ip_str() branch.
function isIpTarget(id: string): boolean {
  const t = id.trim();
  if (/^\d{1,3}(\.\d{1,3}){3}(:\d+)?$/.test(t)) return true;
  if (/^\[[0-9a-fA-F:]+\](:\d+)?$/.test(t)) return true;
  if (/^[0-9a-fA-F:]*:[0-9a-fA-F:]+$/.test(t) && (t.match(/:/g) || []).length >= 2) return true;
  return false;
}

// Add the default direct-access port when the user gave a bare IP.
function normalizeDirectTarget(id: string): string {
  const t = id.trim();
  if (t.startsWith("[")) {
    return t.indexOf("]:") > 0 ? t : t + ":" + DIRECT_PORT;
  }
  return t.indexOf(":") > 0 ? t : t + ":" + DIRECT_PORT;
}

type MsgboxCallback = (type: string, title: string, text: string, link: string) => void;

export default class Connection {
  _msgs: message.DeepPartial<message.Message>[];
  _ws: Websock | undefined;
  _interval: any;
  _id: string;
  _hash: message.Hash | undefined;
  _msgbox: MsgboxCallback;
  _peerInfo: message.PeerInfo | undefined;
  _firstFrame: Boolean | undefined;
  _videoDecoders: { vp9?: VideoDecoder; vp8?: VideoDecoder };
  _videoDecodersReady: Promise<void> | undefined;
  _pendingVideoFrames: message.VideoFrame[];
  _password: Uint8Array | undefined;
  _options: any;
  _videoTestSpeed: number[];
  _closed: boolean;

  constructor() {
    this._msgbox = globals.msgbox;
    this._msgs = [];
    this._id = "";
    this._videoDecoders = {};
    this._pendingVideoFrames = [];
    this._videoTestSpeed = [0, 0];
    this._closed = false;
  }

  async start(id: string) {
    try {
      id = parseIdServerKey(id);
      // IP→/direct only when the delivery opts in (web-direct sets direct: true).
      if (CONF.direct && isIpTarget(id)) {
        await this._startDirect(id);
        return;
      }
      await this._start(id);
    } catch (e: any) {
      if (this._closed) return;
      this.msgbox(
        "error",
        "Connection Error",
        e?.type == "close" ? "Reset by the peer" : String(e)
      );
    }
  }

  // Direct IP access: connect straight to the controlled client's
  // direct-access port through the WS->TCP bridge, skipping rendezvous/relay
  // and the secure handshake (direct connections are plaintext, matching the
  // native client). The controlled side sends the password Hash first, which
  // msgLoop() already handles.
  async _startDirect(target: string) {
    this._loadOptions(target);
    this._startMsgPump();
    const decoders = this.ensureVideoDecoders();
    const addr = normalizeDirectTarget(target);
    const directPath = CONF.directPath || "/direct";
    const uri =
      wsSchema() +
      location.host +
      directPath +
      "?target=" +
      encodeURIComponent(addr);
    console.log(new Date() + ": Direct connecting to " + addr + " via " + uri);
    const ws = new Websock(uri, false);
    this._ws = ws;
    this._id = target;
    await ws.open();
    console.log(new Date() + ": Connected (direct)");
    await this._awaitVideoDecoders(decoders);
    globals.pushEvent("connection_ready", { secure: false, direct: true });
    await this.msgLoop();
  }

  async _start(id: string) {
    this._loadOptions(id);
    this._startMsgPump();
    // Kick off wasm decoder load during rendezvous so it is ready before frames.
    void this.ensureVideoDecoders();
    const uri = getDefaultUri();
    const ws = new Websock(uri, true);
    this._ws = ws;
    this._id = id;
    console.log(new Date() + ": Connecting to rendezvous server: " + uri + ", for " + id);
    await ws.open();
    console.log(new Date() + ": Connected to rendezvous server");
    const punch_hole_request = rendezvous.PunchHoleRequest.fromPartial({
      id,
      licence_key: localStorage.getItem("key") || undefined,
      conn_type: rendezvous.ConnType.DEFAULT_CONN,
      nat_type: rendezvous.NatType.SYMMETRIC,
      token: localStorage.getItem("access_token") || undefined,
    });
    ws.sendRendezvous({ punch_hole_request });
    const msg = (await ws.next()) as rendezvous.RendezvousMessage;
    ws.close();
    console.log(new Date() + ": Got relay response");
    const phr = msg.punch_hole_response;
    const rr = msg.relay_response;
    if (phr) {
      if (phr.other_failure) {
        this.msgbox("error", "Error", phr.other_failure);
        return;
      }
      if (phr.failure != rendezvous.PunchHoleResponse_Failure.UNRECOGNIZED) {
        switch (phr.failure) {
          case rendezvous.PunchHoleResponse_Failure.ID_NOT_EXIST:
            this.msgbox("error", "Error", "ID does not exist");
            break;
          case rendezvous.PunchHoleResponse_Failure.OFFLINE:
            this.msgbox("error", "Error", "Remote desktop is offline");
            break;
          case rendezvous.PunchHoleResponse_Failure.LICENSE_MISMATCH:
            this.msgbox("error", "Error", "Key mismatch");
            break;
          case rendezvous.PunchHoleResponse_Failure.LICENSE_OVERUSE:
            this.msgbox("error", "Error", "Key overuse");
            break;
        }
      }
    } else if (rr) {
      if (!rr.version) {
        this.msgbox("error", "Error", "Remote version is low, not support web");
        return;
      }
      await this.connectRelay(rr);
    }
  }

  _loadOptions(id: string) {
    if (!this._options) {
      this._options = globals.getPeers()[id] || {};
    }
    if (!this._password) {
      const p = this.getOption("password");
      if (p) {
        try {
          this._password = Uint8Array.from(JSON.parse("[" + p + "]"));
        } catch (e) {
          console.error("Failed to get password, " + e);
        }
      }
    }
  }

  _startMsgPump() {
    this._interval = setInterval(() => {
      while (this._msgs.length) {
        this._ws?.sendMessage(this._msgs[0]);
        this._msgs.splice(0, 1);
      }
    }, 1);
  }

  async connectRelay(rr: rendezvous.RelayResponse) {
    const pk = rr.pk;
    let uri = rr.relay_server;
    if (uri) {
      uri = getrUriFromRs(uri, true, 2);
    } else {
      uri = getDefaultUri(true);
    }
    const uuid = rr.uuid;
    console.log(new Date() + ": Connecting to relay server: " + uri);
    const ws = new Websock(uri, false);
    await ws.open();
    console.log(new Date() + ": Connected to relay server");
    this._ws = ws;
    const request_relay = rendezvous.RequestRelay.fromPartial({
      licence_key: localStorage.getItem("key") || undefined,
      uuid,
    });
    ws.sendRendezvous({ request_relay });
    const secure = (await this.secure(pk)) || false;
    await this._awaitVideoDecoders(this.ensureVideoDecoders());
    globals.pushEvent("connection_ready", { secure, direct: false });
    await this.msgLoop();
  }

  async secure(pk: Uint8Array | undefined): Promise<boolean | undefined> {
    if (pk && pk.length) {
      const RS_PK = "OeVuKk5nlHiXp+APNn0Y3pC1Iwpwn44JGqrQCsWqmBw=";
      try {
        pk = await globals.verify(pk, localStorage.getItem("key") || RS_PK);
        if (pk) {
          const idpk = message.IdPk.decode(pk);
          if (idpk.id == this._id) {
            pk = idpk.pk;
          }
        }
        if (pk?.length != 32) {
          pk = undefined;
        }
      } catch (e) {
        console.error("Failed to verify id pk, ", e);
        pk = undefined;
      }
      if (!pk) {
        console.error("Handshake failed: invalid public key from rendezvous server");
      }
    }
    if (!pk) {
      // send an empty message out in case server is setting up secure and
      // waiting for the first message
      const public_key = message.PublicKey.fromPartial({});
      this._ws?.sendMessage({ public_key });
      return;
    }
    const msg = (await this._ws?.next()) as message.Message;
    let signedId: any = msg?.signed_id;
    if (!signedId) {
      console.error("Handshake failed: invalid message type");
      const public_key = message.PublicKey.fromPartial({});
      this._ws?.sendMessage({ public_key });
      return;
    }
    try {
      signedId = await globals.verify(signedId.id, Uint8Array.from(pk!));
    } catch (e) {
      console.error("Failed to verify signed id pk, ", e);
      // fall back to non-secure connection in case pk mismatch
      const public_key = message.PublicKey.fromPartial({});
      this._ws?.sendMessage({ public_key });
      return;
    }
    const idpk = message.IdPk.decode(signedId);
    const id = idpk.id;
    const theirPk = idpk.pk;
    if (id != this._id!) {
      console.error("Handshake failed: sign failure");
      const public_key = message.PublicKey.fromPartial({});
      this._ws?.sendMessage({ public_key });
      return;
    }
    if (theirPk.length != 32) {
      console.error("Handshake failed: invalid public box key length from peer");
      const public_key = message.PublicKey.fromPartial({});
      this._ws?.sendMessage({ public_key });
      return;
    }
    const [mySk, asymmetric_value] = globals.genBoxKeyPair();
    const secret_key = globals.genSecretKey();
    const symmetric_value = globals.seal(secret_key, theirPk, mySk);
    const public_key = message.PublicKey.fromPartial({
      asymmetric_value,
      symmetric_value,
    });
    this._ws?.sendMessage({ public_key });
    this._ws?.setSecretKey(secret_key);
    console.log("secured");
    return true;
  }

  async msgLoop() {
    while (true) {
      const msg = (await this._ws?.next()) as message.Message;
      if (this._closed) break;
      if (!msg) break;
      if (msg.hash) {
        this._hash = msg.hash;
        if (!this._password) this.msgbox("input-password", "Password Required", "");
        this.login();
      } else if (msg.test_delay) {
        const test_delay = msg.test_delay;
        if (!test_delay.from_client) {
          this._ws?.sendMessage({ test_delay });
        }
      } else if (msg.login_response) {
        this.handleLoginResponse(msg.login_response);
      } else if (msg.video_frame) {
        this.handleVideoFrame(msg.video_frame);
      } else if (msg.clipboard) {
        const cb = msg.clipboard;
        if (cb.compress) {
          const c = await decompress(cb.content);
          if (!c) continue;
          cb.content = c;
        }
        try {
          globals.copyToClipboard(new TextDecoder().decode(cb.content));
        } catch (e) {
          console.error("Failed to copy to clipboard, ", e);
        }
      } else if (msg.multi_clipboards) {
        // Multi-clipboard (file clipboard etc.) is not supported in the
        // browser; plain text arrives via `clipboard` above.
      } else if (msg.cursor_data) {
        const cd = msg.cursor_data;
        const c = await decompress(cd.colors);
        if (!c) continue;
        cd.colors = c;
        globals.pushEvent("cursor_data", cd);
      } else if (msg.cursor_id != undefined) {
        globals.pushEvent("cursor_id", { id: msg.cursor_id });
      } else if (msg.cursor_position) {
        globals.pushEvent("cursor_position", msg.cursor_position);
      } else if (msg.misc) {
        if (!this.handleMisc(msg.misc)) break;
      } else if (msg.audio_frame) {
        globals.playAudio(msg.audio_frame.data);
      }
    }
  }

  handleLoginResponse(response: message.LoginResponse) {
    const loginErrorMap: Record<string, any> = {
      [consts.LOGIN_SCREEN_WAYLAND]: {
        msgtype: "error",
        title: "Login Error",
        text: "Login screen using Wayland is not supported",
        link: "https://rustdesk.com/docs/en/manual/linux/#login-screen",
      },
      [consts.LOGIN_MSG_DESKTOP_SESSION_NOT_READY]: {
        msgtype: "session-login",
      },
      [consts.LOGIN_MSG_DESKTOP_XSESSION_FAILED]: {
        msgtype: "session-re-login",
      },
      [consts.LOGIN_MSG_DESKTOP_SESSION_ANOTHER_USER]: {
        msgtype: "info-nocancel",
        title: "another_user_login_title_tip",
        text: "another_user_login_text_tip",
      },
      [consts.LOGIN_MSG_DESKTOP_XORG_NOT_FOUND]: {
        msgtype: "info-nocancel",
        title: "xorg_not_found_title_tip",
        text: "xorg_not_found_text_tip",
        link: "https://rustdesk.com/docs/en/manual/linux/#login-screen",
      },
      [consts.LOGIN_MSG_DESKTOP_NO_DESKTOP]: {
        msgtype: "info-nocancel",
        title: "no_desktop_title_tip",
        text: "no_desktop_text_tip",
        link: "https://rustdesk.com/docs/en/manual/linux/#login-screen",
      },
      [consts.LOGIN_MSG_DESKTOP_SESSION_NOT_READY_PASSWORD_EMPTY]: {
        msgtype: "session-login-password",
      },
      [consts.LOGIN_MSG_DESKTOP_SESSION_NOT_READY_PASSWORD_WRONG]: {
        msgtype: "session-login-re-password",
      },
      [consts.LOGIN_MSG_NO_PASSWORD_ACCESS]: {
        msgtype: "wait-remote-accept-nook",
        title: "Prompt",
        text: "Please wait for the remote side to accept your session request...",
      },
    };

    const err = response.error;
    if (err) {
      if (err == consts.LOGIN_MSG_PASSWORD_EMPTY) {
        this._password = undefined;
        this.msgbox("input-password", "Password Required", "", "");
      } else if (err == consts.LOGIN_MSG_PASSWORD_WRONG) {
        this._password = undefined;
        this.msgbox("re-input-password", err, "Do you want to enter again?");
      } else if (err == consts.LOGIN_MSG_2FA_WRONG || err == consts.REQUIRE_2FA) {
        this.msgbox("input-2fa", err, "");
      } else if (err in loginErrorMap) {
        const m = loginErrorMap[err];
        this.msgbox(m.msgtype, m.title ?? "", m.text ?? "", m.link ?? "");
      } else {
        if (err.includes(consts.SCRAP_X11_REQUIRED)) {
          this.msgbox("error", "Login Error", err, consts.SCRAP_X11_REF_URL);
        } else {
          this.msgbox("error", "Login Error", err);
        }
      }
    } else if (response.peer_info) {
      if (response.enable_trusted_devices) {
        globals.pushEvent("enable_trusted_devices", {});
      }
      this.handlePeerInfo(response.peer_info);
    }
  }

  msgbox(type_: string, title: string, text: string, link: string = "") {
    this._msgbox?.(type_, title, text, link);
  }

  close() {
    this._closed = true;
    this._msgs = [];
    this._pendingVideoFrames = [];
    this._videoDecodersReady = undefined;
    clearInterval(this._interval);
    this._ws?.close();
    this._videoDecoders.vp9?.close();
    this._videoDecoders.vp8?.close();
    this._videoDecoders = {};
  }

  refresh() {
    const misc = message.Misc.fromPartial({ refresh_video: true });
    this._ws?.sendMessage({ misc });
  }

  setMsgbox(callback: MsgboxCallback) {
    this._msgbox = callback;
  }

  login(info?: { os_login?: message.OSLogin; password?: string }) {
    if (info?.password) {
      const salt = this._hash?.salt;
      const p0 = hash([info.password, salt!]);
      this._password = p0;
      const challenge = this._hash?.challenge;
      const p = hash([p0, challenge!]);
      this.msgbox("connecting", "Connecting...", "Logging in...");
      this._sendLoginMessage({ os_login: info.os_login, password: p });
    } else {
      let p = this._password;
      if (p) {
        const challenge = this._hash?.challenge;
        p = hash([p, challenge!]);
      }
      this._sendLoginMessage({ os_login: info?.os_login, password: p });
    }
  }

  changePreferCodec() {
    const option = message.OptionMessage.fromPartial({
      supported_decoding: this.webSupportedDecoding(),
    });
    const misc = message.Misc.fromPartial({ option });
    this._ws?.sendMessage({ misc });
  }

  webSupportedDecoding(): message.SupportedDecoding {
    return message.SupportedDecoding.fromPartial(webSupportedDecodingPartial());
  }

  async reconnect() {
    const id = this._id;
    this.close();
    this._closed = false;
    await this.start(id);
  }

  _sendLoginMessage(login: { os_login?: message.OSLogin; password?: Uint8Array }) {
    const login_request = message.LoginRequest.fromPartial({
      username: this._id!,
      my_id: "web",
      my_name: "web",
      my_platform: "Web",
      version,
      hwid: globals.getHwid(),
      password: login.password,
      option: this.getOptionMessage(),
      video_ack_required: true,
      os_login: login.os_login,
    });
    this._ws?.sendMessage({ login_request });
  }

  getOptionMessage(): message.OptionMessage | undefined {
    let n = 0;
    const msg = message.OptionMessage.fromPartial({});
    // Always advertise VP8/VP9, matching native LoginRequest.option.
    // ogv.js cannot paint H264/AV1; without this the host may pick those.
    msg.supported_decoding = this.webSupportedDecoding();
    n += 1;
    const q = this.getImageQualityEnum(this.getImageQuality(), true);
    const yes = message.OptionMessage_BoolOption.Yes;
    if (q != undefined) {
      msg.image_quality = q;
      n += 1;
    }
    if (this._options["show-remote-cursor"]) {
      msg.show_remote_cursor = yes;
      n += 1;
    }
    if (this._options["lock-after-session-end"]) {
      msg.lock_after_session_end = yes;
      n += 1;
    }
    if (this._options["privacy-mode"]) {
      msg.privacy_mode = yes;
      n += 1;
    }
    if (this._options["disable-audio"]) {
      msg.disable_audio = yes;
      n += 1;
    }
    if (this._options["disable-clipboard"]) {
      msg.disable_clipboard = yes;
      n += 1;
    }
    return n > 0 ? msg : undefined;
  }

  sendVideoReceived() {
    const misc = message.Misc.fromPartial({ video_received: true });
    this._ws?.sendMessage({ misc });
  }

  handleVideoFrame(vf: message.VideoFrame) {
    if (!this._firstFrame) {
      this.msgbox("", "", "");
      this._firstFrame = true;
    }
    const kind = videoFrameKind(vf);
    const dec =
      kind === "vp9"
        ? this._videoDecoders.vp9
        : kind === "vp8"
          ? this._videoDecoders.vp8
          : undefined;
    const action = videoFrameAction(kind, !!dec);
    if (action.type === "queue") {
      this._pendingVideoFrames = enqueueVideoFrame(this._pendingVideoFrames, vf);
      // video_ack_required is set on login; dropping the first frame without
      // an ack lets the host stall after the keyframe.
      this.sendVideoReceived();
      void this.ensureVideoDecoders();
      return;
    }
    if (action.type === "renegotiate") {
      console.warn(
        "Unsupported video codec " + action.kind + "; requesting VP8/VP9"
      );
      this.changePreferCodec();
      this.refresh();
      this.sendVideoReceived();
      return;
    }
    if (action.type !== "decode" || !dec) return;
    const frames = action.kind === "vp9" ? vf.vp9s : vf.vp8s;
    if (!frames) return;
    const tm = new Date().getTime();
    let i = 0;
    const n = frames.frames.length;
    frames.frames.forEach((f) => {
      dec.processFrame(f.data.slice(0).buffer, (ok: boolean) => {
        i++;
        if (i == n) this.sendVideoReceived();
        if (ok && dec.frameBuffer && n == i) {
          globals.draw(vf.display, dec.frameBuffer);
          const now = new Date().getTime();
          this._videoTestSpeed[1] += now - tm;
          this._videoTestSpeed[0] += 1;
          if (this._videoTestSpeed[0] >= 30) {
            console.log(
              "video decoder: " +
                parseInt("" + this._videoTestSpeed[1] / this._videoTestSpeed[0])
            );
            this._videoTestSpeed = [0, 0];
          }
        }
      });
    });
  }

  handlePeerInfo(pi: message.PeerInfo) {
    localStorage.setItem("last_remote_id", this._id);
    this._peerInfo = pi;
    if (pi.current_display > pi.displays.length) {
      pi.current_display = 0;
    }
    if (globals.getVersionNumber(pi.version) < globals.getVersionNumber("1.1.10")) {
      this.setPermission("restart", false);
    }
    if (pi.displays.length == 0) {
      this.setOption("info", pi);
      globals.pushEvent("update_privacy_mode", {});
      this.msgbox("error", "Remote Error", "No Display");
      return;
    }
    this.msgbox("success", "Successful", "Connected, waiting for image...");
    globals.pushEvent("peer_info", pi);
    // Repeat VP8/VP9 abilities after login (native update_supported_decodings).
    this.changePreferCodec();
    const p = this.shouldAutoLogin();
    if (p) this.inputOsPassword(p);
    const username = this.getOption("info")?.username;
    if (username && !pi.username) pi.username = username;
    globals.pushEvent("update_privacy_mode", {});
    this.setOption("info", pi);
    if (this.getRemember()) {
      if (this._password?.length) {
        const p = this._password.toString();
        if (p != this.getOption("password")) {
          this.setOption("password", p);
        }
      }
    } else {
      this.setOption("password", undefined);
    }
  }

  setPermission(name: string, value: Boolean) {
    globals.pushEvent("permission", { [name]: value });
  }

  shouldAutoLogin(): string {
    const l = this.getOption("lock-after-session-end");
    const a = !!this.getOption("auto-login");
    const p = this.getOption("os-password");
    if (p && l && a) {
      return p;
    }
    return "";
  }

  handleMisc(misc: message.Misc): boolean {
    if (misc.audio_format) {
      globals.initAudio(misc.audio_format.channels, misc.audio_format.sample_rate);
    } else if (misc.chat_message) {
      globals.pushEvent("chat_client_mode", { text: misc.chat_message.text });
    } else if (misc.permission_info) {
      const p = misc.permission_info;
      console.info("Change permission " + p.permission + " -> " + p.enabled);
      let name;
      switch (p.permission) {
        case message.PermissionInfo_Permission.Keyboard:
          name = "keyboard";
          break;
        case message.PermissionInfo_Permission.Clipboard:
          name = "clipboard";
          break;
        case message.PermissionInfo_Permission.Audio:
          name = "audio";
          break;
        default:
          return true;
      }
      this.setPermission(name, p.enabled);
    } else if (misc.switch_display) {
      this._videoDecoders.vp9?.close();
      this._videoDecoders.vp8?.close();
      this._videoDecoders = {};
      this._videoDecodersReady = undefined;
      void this.ensureVideoDecoders();
      globals.pushEvent("switch_display", misc.switch_display);
    } else if (misc.close_reason) {
      this.msgbox("error", "Connection Error", misc.close_reason);
      this.close();
      return false;
    } else if (misc.elevation_response) {
      globals.pushEvent("show_elevation", { show: misc.elevation_response == "No" });
    }
    return true;
  }

  getRemember(): Boolean {
    return this._options["remember"] || false;
  }

  setRemember(v: Boolean) {
    this.setOption("remember", v);
  }

  getOption(name: string): any {
    return this._options[name] ?? globals.getUserDefaultOption(name);
  }

  getToggleOption(name: string): Boolean {
    const defaultToggleTrue = [
      "show-remote-cursor",
      "privacy-mode",
      "enable-file-copy-paste",
      "allow_swap_key",
    ];
    return this._options[name] || (defaultToggleTrue.includes(name) ? true : false);
  }

  getStatus(): String {
    return JSON.stringify({ status_num: 10 });
  }

  setOption(name: string, value: any) {
    if (value == undefined) {
      delete this._options[name];
    } else {
      this._options[name] = value;
    }
    this._options["tm"] = new Date().getTime();
    const peers = globals.getPeers();
    peers[this._id] = this._options;
    localStorage.setItem("peers", JSON.stringify(peers));
  }

  inputKey(
    name: string,
    down: boolean,
    press: boolean,
    alt: Boolean,
    ctrl: Boolean,
    shift: Boolean,
    command: Boolean
  ) {
    const key_event = mapKey(name, globals.isDesktop());
    if (!key_event) return;
    if (alt && (name == "VK_MENU" || name == "RAlt")) alt = false;
    if (ctrl && (name == "VK_CONTROL" || name == "RControl")) ctrl = false;
    if (shift && (name == "VK_SHIFT" || name == "RShift")) shift = false;
    if (command && (name == "Meta" || name == "RWin")) command = false;
    key_event.down = down;
    key_event.press = press;
    key_event.modifiers = this.getMod(alt, ctrl, shift, command);
    this._ws?.sendMessage({ key_event });
  }

  ctrlAltDel() {
    const key_event = message.KeyEvent.fromPartial({ down: true });
    if (this._peerInfo?.platform == "Windows") {
      key_event.control_key = message.ControlKey.CtrlAltDel;
    } else {
      key_event.control_key = message.ControlKey.Delete;
      key_event.modifiers = this.getMod(true, true, false, false);
    }
    this._ws?.sendMessage({ key_event });
  }

  restart() {
    const misc = message.Misc.fromPartial({ restart_remote_device: true });
    this._ws?.sendMessage({ misc });
  }

  inputString(seq: string) {
    const key_event = message.KeyEvent.fromPartial({ seq });
    this._ws?.sendMessage({ key_event });
  }

  send2fa(code: string) {
    const auth_2fa = message.Auth2FA.fromPartial({ code, hwid: globals.getHwid() });
    this._ws?.sendMessage({ auth_2fa });
  }

  _captureDisplays({ add, sub, set }: { add?: number[]; sub?: number[]; set?: number[] }) {
    const capture_displays = message.CaptureDisplays.fromPartial({ add, sub, set });
    const misc = message.Misc.fromPartial({ capture_displays });
    this._ws?.sendMessage({ misc });
  }

  switchDisplay(v: string) {
    try {
      const obj = JSON.parse(v);
      const value: number[] = obj.value;
      const isDesktop = obj.isDesktop;
      if (value.length == 1) {
        const switch_display = message.SwitchDisplay.fromPartial({ display: value[0] });
        const misc = message.Misc.fromPartial({ switch_display });
        this._ws?.sendMessage({ misc });
        if (!isDesktop) {
          this._captureDisplays({ set: value });
        }
      } else {
        this._captureDisplays({ set: value });
      }
    } catch (e) {
      console.log('Failed to switch display, invalid param "' + v + '"');
    }
  }

  elevateDirect() {
    const elevation_request = message.ElevationRequest.fromPartial({ direct: true });
    const misc = message.Misc.fromPartial({ elevation_request });
    this._ws?.sendMessage({ misc });
  }

  elevateWithLogon(value: string) {
    try {
      const obj = JSON.parse(value);
      const logon = message.ElevationRequestWithLogon.fromPartial({
        username: obj.username,
        password: obj.password,
      });
      const elevation_request = message.ElevationRequest.fromPartial({ logon });
      const misc = message.Misc.fromPartial({ elevation_request });
      this._ws?.sendMessage({ misc });
    } catch (e) {
      console.log('Failed to elevate with logon, invalid param "' + value + '"');
    }
  }

  async inputOsPassword(seq: string) {
    this.inputMouse();
    await sleep(50);
    this.inputMouse(0, 3, 3);
    await sleep(50);
    this.inputMouse(1 | (1 << 3));
    this.inputMouse(2 | (1 << 3));
    await sleep(1200);
    const key_event = message.KeyEvent.fromPartial({ press: true, seq });
    this._ws?.sendMessage({ key_event });
  }

  lockScreen() {
    const key_event = message.KeyEvent.fromPartial({
      down: true,
      control_key: message.ControlKey.LockScreen,
    });
    this._ws?.sendMessage({ key_event });
  }

  getMod(alt: Boolean, ctrl: Boolean, shift: Boolean, command: Boolean) {
    const mod: message.ControlKey[] = [];
    if (alt) mod.push(message.ControlKey.Alt);
    if (ctrl) mod.push(message.ControlKey.Control);
    if (shift) mod.push(message.ControlKey.Shift);
    if (command) mod.push(message.ControlKey.Meta);
    return mod;
  }

  inputMouse(
    mask: number = 0,
    x: number = 0,
    y: number = 0,
    alt: Boolean = false,
    ctrl: Boolean = false,
    shift: Boolean = false,
    command: Boolean = false
  ) {
    const mouse_event = message.MouseEvent.fromPartial({
      mask,
      x,
      y,
      modifiers: this.getMod(alt, ctrl, shift, command),
    });
    this._ws?.sendMessage({ mouse_event });
  }

  toggleOption(name: string) {
    const v = !this._options[name];
    const option = message.OptionMessage.fromPartial({});
    const v2 = v ? message.OptionMessage_BoolOption.Yes : message.OptionMessage_BoolOption.No;
    switch (name) {
      case "show-remote-cursor":
        option.show_remote_cursor = v2;
        break;
      case "disable-audio":
        option.disable_audio = v2;
        break;
      case "disable-clipboard":
        option.disable_clipboard = v2;
        break;
      case "lock-after-session-end":
        option.lock_after_session_end = v2;
        break;
      case "privacy-mode":
        option.privacy_mode = v2;
        break;
      case "enable-file-copy-paste":
        option.enable_file_transfer = v2;
        break;
      case "block-input":
        option.block_input = message.OptionMessage_BoolOption.Yes;
        break;
      case "unblock-input":
        option.block_input = message.OptionMessage_BoolOption.No;
        break;
      case "show-quality-monitor":
      case "allow-swap-key":
        break;
      case "view-only":
        if (v) {
          option.disable_keyboard = message.OptionMessage_BoolOption.Yes;
          option.disable_clipboard = message.OptionMessage_BoolOption.Yes;
          option.show_remote_cursor = message.OptionMessage_BoolOption.Yes;
          option.enable_file_transfer = message.OptionMessage_BoolOption.No;
          option.lock_after_session_end = message.OptionMessage_BoolOption.No;
        } else {
          option.disable_keyboard = message.OptionMessage_BoolOption.No;
          option.disable_clipboard = this.getToggleOption("disable-clipboard")
            ? message.OptionMessage_BoolOption.Yes
            : message.OptionMessage_BoolOption.No;
          option.show_remote_cursor = this.getToggleOption("show-remote-cursor")
            ? message.OptionMessage_BoolOption.Yes
            : message.OptionMessage_BoolOption.No;
          option.enable_file_transfer = this.getToggleOption("enable-file-copy-paste")
            ? message.OptionMessage_BoolOption.Yes
            : message.OptionMessage_BoolOption.No;
          option.lock_after_session_end = this.getToggleOption("lock-after-session-end")
            ? message.OptionMessage_BoolOption.Yes
            : message.OptionMessage_BoolOption.No;
        }
        break;
      default:
        this.setOption(name, this._options[name] ? undefined : "Y");
        return;
    }
    if (name.indexOf("block-input") < 0) this.setOption(name, v);
    const misc = message.Misc.fromPartial({ option });
    this._ws?.sendMessage({ misc });
  }

  togglePrivacyMode(value: string) {
    try {
      const obj = JSON.parse(value);
      const toggle_privacy_mode = message.TogglePrivacyMode.fromPartial({
        impl_key: obj.impl_key,
        on: obj.on,
      });
      const misc = message.Misc.fromPartial({ toggle_privacy_mode });
      this._ws?.sendMessage({ misc });
    } catch (e) {
      console.log('Failed to toggle privacy mode, invalid param "' + value + '"');
    }
  }

  toggleVirtualDisplay(value: string) {
    try {
      const obj = JSON.parse(value);
      const toggle_virtual_display = message.ToggleVirtualDisplay.fromPartial({
        display: obj.index,
        on: obj.on,
      });
      const misc = message.Misc.fromPartial({ toggle_virtual_display });
      this._ws?.sendMessage({ misc });
    } catch (e) {
      console.log('Failed to toggle virtual display, invalid param "' + value + '"');
    }
  }

  changeResolution(value: string) {
    try {
      const obj = JSON.parse(value);
      const change_display_resolution = message.DisplayResolution.fromPartial({
        display: obj.display,
        resolution: { width: obj.width, height: obj.height },
      });
      const misc = message.Misc.fromPartial({ change_display_resolution });
      this._ws?.sendMessage({ misc });
    } catch (e) {
      console.log('Failed to change resolution, invalid param "' + value + '"');
    }
  }

  sendSelectedSessionId(sid: string) {
    const selected_sid = parseInt(sid);
    if (isNaN(selected_sid)) return;
    const misc = message.Misc.fromPartial({ selected_sid });
    this._ws?.sendMessage({ misc });
  }

  getImageQuality() {
    return this.getOption("image-quality");
  }

  getImageQualityEnum(value: string, ignoreDefault: Boolean): message.ImageQuality | undefined {
    switch (value) {
      case "low":
        return message.ImageQuality.Low;
      case "best":
        return message.ImageQuality.Best;
      case "balanced":
        return ignoreDefault ? undefined : message.ImageQuality.Balanced;
      default:
        return undefined;
    }
  }

  setImageQuality(value: string) {
    this.setOption("image-quality", value);
    const image_quality = this.getImageQualityEnum(value, false);
    if (image_quality == undefined) return;
    const option = message.OptionMessage.fromPartial({ image_quality });
    const misc = message.Misc.fromPartial({ option });
    this._ws?.sendMessage({ misc });
  }

  setCustomImageQuality(value: number) {
    const custom_image_quality = parseInt(String(value));
    if (isNaN(custom_image_quality)) return;
    const option = message.OptionMessage.fromPartial({ custom_image_quality });
    const misc = message.Misc.fromPartial({ option });
    this._ws?.sendMessage({ misc });
  }

  setCustomFps(value: number) {
    const custom_fps = parseInt(String(value));
    if (isNaN(custom_fps)) return;
    const option = message.OptionMessage.fromPartial({ custom_fps });
    const misc = message.Misc.fromPartial({ option });
    this._ws?.sendMessage({ misc });
  }

  ensureVideoDecoders(): Promise<void> {
    if (this._videoDecoders.vp9 && this._videoDecoders.vp8) {
      return Promise.resolve();
    }
    if (!this._videoDecodersReady) {
      this._videoDecodersReady = this._loadVideoDecoders().catch((e) => {
        this._videoDecodersReady = undefined;
        throw e;
      });
    }
    return this._videoDecodersReady;
  }

  async _awaitVideoDecoders(p: Promise<void>) {
    try {
      await Promise.race([
        p,
        new Promise<void>((_, reject) =>
          setTimeout(() => reject(new Error("decoder load timeout")), 8000)
        ),
      ]);
    } catch (e) {
      console.error("Failed to load video decoders, " + e);
    }
  }

  async _loadVideoDecoders() {
    this._videoDecoders.vp9?.close();
    this._videoDecoders.vp8?.close();
    this._videoDecoders = {};
    const [vp9, vp8] = await Promise.all([
      loadVideoDecoder("vp9"),
      loadVideoDecoder("vp8"),
    ]);
    if (this._closed) {
      vp9.close();
      vp8.close();
      return;
    }
    this._videoDecoders.vp9 = vp9;
    this._videoDecoders.vp8 = vp8;
    const pending = this._pendingVideoFrames;
    this._pendingVideoFrames = [];
    for (const vf of pending) {
      this.handleVideoFrame(vf);
    }
  }
}

// The connection page accepts "<id>@<server>?key=<key>" (same syntax as the
// native client). Split it, persisting server/key like the settings page does.
function parseIdServerKey(id: string): string {
  let t = id.trim();
  const qi = t.indexOf("?");
  if (qi >= 0) {
    const params = new URLSearchParams(t.substring(qi + 1));
    const key = params.get("key");
    if (key) localStorage.setItem("key", key);
    t = t.substring(0, qi);
  }
  const at = t.indexOf("@");
  if (at >= 0) {
    const server = t.substring(at + 1);
    if (server) localStorage.setItem("custom-rendezvous-server", server);
    t = t.substring(0, at);
  }
  return t;
}

function getDefaultUri(isRelay: Boolean = false): string {
  const host = localStorage.getItem("custom-rendezvous-server");
  return getrUriFromRs(host || CONF.server || "", isRelay);
}

// Server address resolution, mirroring check_ws() of the native client
// (libs/hbb_common/src/websocket.rs):
//   ""               -> same origin, path based (the page is served by a
//                       WS-capable proxy: rustdesk-web binary or Caddy)
//   domain           -> ws(s)://domain/ws/id | /ws/relay (reverse proxy on 80/443)
//   domain:port      -> ws(s)://domain:(port+2 | +3) (direct WS ports)
//   ip[:port]        -> ws(s)://ip:(port+2 | +3)
function getrUriFromRs(uri: string, isRelay: Boolean = false, roffset: number = 0): string {
  const wsPath = isRelay ? CONF.wsRelayPath || "/ws/relay" : CONF.wsIdPath || "/ws/id";
  if (!uri) {
    return wsSchema() + location.host + wsPath;
  }
  let host = uri;
  let port = 0;
  const i = uri.lastIndexOf(":");
  if (i > 0 && uri.indexOf(":") == i) {
    host = uri.substring(0, i);
    port = parseInt(uri.substring(i + 1)) || 0;
  }
  // "localhost" resolves to a loopback IP; the native client treats it as an
  // IP (direct ports), not as a domain behind a reverse proxy.
  const isIp =
    /^(\d{1,3}\.){3}\d{1,3}$/.test(host) ||
    host.indexOf(":") >= 0 ||
    host === "localhost";
  if (!isIp && !port) {
    return wsSchema() + host + wsPath;
  }
  const base = port || PORT;
  return wsSchema() + host + ":" + (base + (isRelay ? roffset || 3 : 2));
}

function hash(datas: (string | Uint8Array)[]): Uint8Array {
  const hasher = new sha256.Hash();
  datas.forEach((data) => {
    if (typeof data == "string") {
      data = new TextEncoder().encode(data);
    }
    return hasher.update(data);
  });
  return hasher.digest();
}
