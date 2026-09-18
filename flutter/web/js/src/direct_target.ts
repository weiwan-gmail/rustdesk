// Direct-mode target helpers used by connection.ts (_startDirect), first-visit
// Remote ID prefill, and tests. Loopback names map to 127.0.0.1 for connect
// and prefill. LAN/FQDN hostnames are connect-as-typed; the Go /direct bridge
// DNS-resolves them and applies the IP allowlist. Prefill still does not use
// LAN hostnames (page host / --default-target only).

export const DIRECT_PORT = 21118;

const LOOPBACK = /^(localhost|127\.0\.0\.1|\[?::1\]?)(:\d+)?$/i;
const IPV4 = /^(\d{1,3}\.){3}\d{1,3}$/;

export function isDirectTarget(id: string): boolean {
  const t = id.trim();
  return isIpTarget(t) || isHostnameTarget(t);
}

export function isIpTarget(id: string): boolean {
  const t = id.trim();
  if (LOOPBACK.test(t)) return true;
  if (/^\d{1,3}(\.\d{1,3}){3}(:\d+)?$/.test(t)) return true;
  if (/^\[[0-9a-fA-F:]+\](:\d+)?$/.test(t)) return true;
  if (
    /^[0-9a-fA-F:]*:[0-9a-fA-F:]+$/.test(t) &&
    (t.match(/:/g) || []).length >= 2
  ) {
    return true;
  }
  return false;
}

export function isHostnameTarget(id: string): boolean {
  const t = id.trim();
  if (!t || isIpTarget(t)) return false;
  const { host, port } = splitHostPort(t);
  if (port !== undefined && !validPort(port)) return false;
  return validHostname(host);
}

export function normalizeDirectTarget(id: string): string {
  let t = id.trim();
  const loopback = LOOPBACK.exec(t);
  if (loopback) {
    t = "127.0.0.1" + (loopback[2] || "");
  }
  if (t.startsWith("[")) {
    return t.indexOf("]:") > 0 ? t : t + ":" + DIRECT_PORT;
  }
  const colon = t.indexOf(":");
  if (colon > 0 && t.indexOf(":") === colon) {
    return t;
  }
  return t + ":" + DIRECT_PORT;
}

export function usefulDirectConnectHost(host: string): string | null {
  let h = host.trim();
  if (!h) return null;
  if (h.startsWith("[") && h.endsWith("]") && h.length > 2) {
    h = h.slice(1, -1);
  }
  const lower = h.toLowerCase();
  if (lower === "localhost" || lower === "127.0.0.1" || lower === "::1") {
    return "127.0.0.1";
  }
  if (IPV4.test(h)) return h;
  if (h.includes(":")) return h;
  return null;
}

export function resolveWebDirectRemoteId(opts: {
  isWeb: boolean;
  direct: boolean;
  lastRemoteId: string;
  defaultTarget: string;
  locationHost: string;
}): string {
  const saved = opts.lastRemoteId.trim();
  if (saved) return saved;
  if (!opts.isWeb || !opts.direct) return "";
  const fromConfig = opts.defaultTarget.trim();
  if (fromConfig) return fromConfig;
  return usefulDirectConnectHost(opts.locationHost) || "";
}

function splitHostPort(t: string): { host: string; port?: string } {
  if (t.startsWith("[")) {
    const end = t.indexOf("]");
    if (end < 0) return { host: t };
    const rest = t.slice(end + 1);
    if (rest.startsWith(":")) return { host: t.slice(1, end), port: rest.slice(1) };
    return { host: t.slice(1, end) };
  }
  const i = t.lastIndexOf(":");
  if (i > 0 && t.indexOf(":") === i) {
    return { host: t.slice(0, i), port: t.slice(i + 1) };
  }
  return { host: t };
}

function validPort(port: string): boolean {
  if (!/^\d{1,5}$/.test(port)) return false;
  const n = Number(port);
  return n > 0 && n <= 65535;
}

function validHostname(host: string): boolean {
  if (!host || host.length > 253 || /^\d+$/.test(host)) return false;
  const trimmed = host.endsWith(".") ? host.slice(0, -1) : host;
  if (!trimmed) return false;
  const labels = trimmed.split(".");
  let lastHasLetter = false;
  for (const label of labels) {
    if (!validHostnameLabel(label)) return false;
    lastHasLetter = /[A-Za-z]/.test(label);
  }
  return lastHasLetter;
}

function validHostnameLabel(label: string): boolean {
  if (!label || label.length > 63) return false;
  if (label.startsWith("-") || label.endsWith("-")) return false;
  return /^[A-Za-z0-9-]+$/.test(label);
}
