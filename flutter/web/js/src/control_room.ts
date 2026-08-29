// Optional exclusive control room among web viewers of the same target.
// Isolated from the protocol stack: connection.ts only calls attach/detach
// after a successful login when window.RUSTDESK_CONFIG.control is true.
// No import-time side effects (no WebSocket, no DOM, no option writes).

export const LS_BAR = "rd-control-bar";
export const LS_AUTO = "rd-control-auto-approve";

export type ControlConn = {
  _id: string;
  _closed: boolean;
  setViewOnly: (enabled: boolean) => void;
};

type RoomState = {
  type?: string;
  you?: string;
  controllerIp?: string;
  viewerCount?: number;
  memberCount?: number;
  pendingIp?: string;
  youRequested?: boolean;
  autoApprove?: boolean;
  globalAutoApprove?: boolean;
};

type Session = {
  conn: ControlConn;
  ws?: WebSocket;
  stopped: boolean;
  timer?: ReturnType<typeof setTimeout>;
  attempt: number;
  lastYou: string;
  el?: HTMLElement;
  last?: RoomState;
};

const sessions = new WeakMap<ControlConn, Session>();
const forcedViewers = new WeakSet<ControlConn>();

function rustdeskConfig(): any {
  const g = globalThis as any;
  return g.RUSTDESK_CONFIG || g.window?.RUSTDESK_CONFIG || {};
}

export function controlEnabled(): boolean {
  return !!rustdeskConfig().control;
}

export function controlBarEnabled(): boolean {
  if (!controlEnabled()) return false;
  return rustdeskConfig().controlBar !== false;
}

export function isForcedViewer(conn: ControlConn): boolean {
  return forcedViewers.has(conn);
}

export function attachControlRoom(conn: ControlConn) {
  if (!controlEnabled()) return;
  detachControlRoom(conn);
  const s: Session = { conn, stopped: false, attempt: 0, lastYou: "" };
  sessions.set(conn, s);
  connect(s);
}

export function detachControlRoom(conn: ControlConn) {
  const s = sessions.get(conn);
  if (!s) {
    forcedViewers.delete(conn);
    return;
  }
  s.stopped = true;
  if (s.timer) clearTimeout(s.timer);
  try {
    s.ws?.close();
  } catch {
    // ignore
  }
  s.el?.remove();
  sessions.delete(conn);
  forcedViewers.delete(conn);
}

function connect(s: Session) {
  if (s.stopped || s.conn._closed) return;
  const cfg = rustdeskConfig();
  const path = cfg.controlPath || "/control";
  const proto = typeof location !== "undefined" && location.protocol === "https:" ? "wss://" : "ws://";
  const host = typeof location !== "undefined" ? location.host : "localhost";
  const auto = storageGet(LS_AUTO) === "1";
  const uri =
    proto +
    host +
    path +
    "?target=" +
    encodeURIComponent(s.conn._id) +
    (auto ? "&autoApprove=1" : "");
  let ws: WebSocket;
  try {
    ws = new WebSocket(uri);
  } catch {
    scheduleReconnect(s);
    return;
  }
  s.ws = ws;
  ws.onmessage = (ev) => {
    if (s.stopped) return;
    s.attempt = 0;
    let st: RoomState;
    try {
      st = JSON.parse(String(ev.data));
    } catch {
      return;
    }
    applyState(s, st);
  };
  ws.onerror = () => {
    // onclose follows
  };
  ws.onclose = () => {
    if (s.stopped || s.conn._closed) return;
    forceViewer(s.conn);
    scheduleReconnect(s);
  };
}

function scheduleReconnect(s: Session) {
  if (s.stopped || s.conn._closed) return;
  const delay = Math.min(1000 * Math.pow(2, s.attempt), 8000);
  s.attempt += 1;
  s.timer = setTimeout(() => connect(s), delay);
}

function applyState(s: Session, st: RoomState) {
  s.last = st;
  const you = st.you === "controller" ? "controller" : "viewer";
  if (you !== s.lastYou) {
    s.lastYou = you;
    if (you === "viewer") {
      forceViewer(s.conn);
    } else {
      forcedViewers.delete(s.conn);
      s.conn.setViewOnly(false);
    }
  }
  renderBar(s);
}

function forceViewer(conn: ControlConn) {
  forcedViewers.add(conn);
  try {
    conn.setViewOnly(true);
  } catch {
    // ignore
  }
}

function send(s: Session, obj: object) {
  if (s.ws && s.ws.readyState === 1) {
    s.ws.send(JSON.stringify(obj));
  }
}

function storageGet(k: string): string | null {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
}

function storageSet(k: string, v: string | null) {
  try {
    if (v === null) localStorage.removeItem(k);
    else localStorage.setItem(k, v);
  } catch {
    // ignore
  }
}

type Copy = {
  controlling: string;
  viewing: string;
  watching: string;
  request: string;
  waiting: string;
  approve: string;
  deny: string;
  release: string;
  autoNext: string;
  collapse: string;
};

export function barCopy(lang?: string): Copy {
  const l = (lang || (typeof navigator !== "undefined" ? navigator.language : "") || "").toLowerCase();
  if (l.startsWith("zh")) {
    return {
      controlling: "控制中",
      viewing: "观看",
      watching: "人观看",
      request: "申请控制",
      waiting: "等待批准",
      approve: "批准",
      deny: "拒绝",
      release: "释放",
      autoNext: "下次自动批准",
      collapse: "收起",
    };
  }
  return {
    controlling: "Controlling",
    viewing: "Viewing",
    watching: "watching",
    request: "Request control",
    waiting: "Waiting for approval",
    approve: "Approve",
    deny: "Deny",
    release: "Release",
    autoNext: "Auto-approve next",
    collapse: "Hide",
  };
}

function ensureStyle() {
  if (typeof document === "undefined") return;
  if (document.getElementById("rd-cr-style")) return;
  const st = document.createElement("style");
  st.id = "rd-cr-style";
  st.textContent = `
#rd-cr-bar, #rd-cr-dot {
  position: fixed; left: 50%; bottom: 12px; transform: translateX(-50%);
  z-index: 2147483000; pointer-events: none;
  font: 13px/1.2 system-ui, sans-serif; color: #f2f2f2;
}
#rd-cr-bar {
  display: flex; align-items: center; gap: 8px;
  background: rgba(0,0,0,0.45); border-radius: 16px;
  padding: 6px 12px; max-width: 90vw; height: 32px; box-sizing: border-box;
  backdrop-filter: blur(4px);
}
#rd-cr-dot {
  width: 14px; height: 14px; border-radius: 50%;
  background: rgba(0,0,0,0.45); cursor: pointer; pointer-events: auto;
}
#rd-cr-bar button, #rd-cr-bar label, #rd-cr-bar input {
  pointer-events: auto;
}
#rd-cr-bar button {
  background: rgba(255,255,255,0.18); color: #fff; border: 0;
  border-radius: 12px; padding: 3px 10px; cursor: pointer;
}
#rd-cr-bar button:hover { background: rgba(255,255,255,0.28); }
#rd-cr-bar label { display: inline-flex; align-items: center; gap: 4px; cursor: pointer; }
#rd-cr-ip { opacity: 0.9; }
`;
  document.head.appendChild(st);
}

function renderBar(s: Session) {
  if (!controlBarEnabled() || typeof document === "undefined") return;
  ensureStyle();
  const collapsed = storageGet(LS_BAR) === "0";
  if (collapsed) {
    s.el?.remove();
    let dot = document.getElementById("rd-cr-dot");
    if (!dot) {
      dot = document.createElement("div");
      dot.id = "rd-cr-dot";
      dot.title = "control";
      document.body.appendChild(dot);
      dot.addEventListener("click", () => {
        storageSet(LS_BAR, null);
        const cur = sessions.get(s.conn);
        if (cur) renderBar(cur);
      });
    }
    return;
  }
  document.getElementById("rd-cr-dot")?.remove();
  if (!s.el) {
    s.el = document.createElement("div");
    s.el.id = "rd-cr-bar";
    document.body.appendChild(s.el);
    s.el.addEventListener("click", (ev) => {
      const t = ev.target as HTMLElement;
      const act = t.getAttribute("data-rd");
      if (!act) return;
      if (act === "request") send(s, { type: "request" });
      if (act === "release") send(s, { type: "release" });
      if (act === "approve") send(s, { type: "approve", autoApprove: autoChecked() });
      if (act === "deny") send(s, { type: "deny" });
      if (act === "hide") {
        storageSet(LS_BAR, "0");
        renderBar(s);
      }
    });
    s.el.addEventListener("change", (ev) => {
      const t = ev.target as HTMLInputElement;
      if (t && t.getAttribute("data-rd") === "auto") {
        storageSet(LS_AUTO, t.checked ? "1" : null);
        send(s, { type: "setAutoApprove", autoApprove: t.checked });
      }
    });
  }
  const st = s.last || {};
  const t = barCopy();
  const you = st.you === "controller" ? "controller" : "viewer";
  const bits: string[] = [];
  bits.push(`<span>${you === "controller" ? t.controlling : t.viewing}</span>`);
  bits.push(`<span>${Number(st.viewerCount) || 0} ${t.watching}</span>`);
  if (st.controllerIp) {
    bits.push(`<span id="rd-cr-ip">${esc(st.controllerIp)}</span>`);
  }
  if (you === "viewer") {
    if (st.youRequested) {
      bits.push(`<span>${t.waiting}</span>`);
    } else {
      bits.push(`<button type="button" data-rd="request">${t.request}</button>`);
    }
  } else {
    bits.push(`<button type="button" data-rd="release">${t.release}</button>`);
    if (st.pendingIp) {
      bits.push(`<span id="rd-cr-ip">${esc(st.pendingIp)}</span>`);
      bits.push(`<button type="button" data-rd="approve">${t.approve}</button>`);
      bits.push(`<button type="button" data-rd="deny">${t.deny}</button>`);
    }
    const checked = st.autoApprove || st.globalAutoApprove || storageGet(LS_AUTO) === "1";
    bits.push(
      `<label><input type="checkbox" data-rd="auto"${checked ? " checked" : ""}>${t.autoNext}</label>`
    );
  }
  bits.push(`<button type="button" data-rd="hide">${t.collapse}</button>`);
  s.el.innerHTML = bits.join("");
}

function autoChecked(): boolean {
  return storageGet(LS_AUTO) === "1";
}

export function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => {
    switch (c) {
      case "&":
        return "&amp;";
      case "<":
        return "&lt;";
      case ">":
        return "&gt;";
      case '"':
        return "&quot;";
      case "'":
        return "&#39;";
      default:
        return c;
    }
  });
}
