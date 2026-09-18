// Direct-mode target helpers used by connection.ts (_startDirect) and tests.
// Loopback names are connect-as-IP: localhost / ::1 map to 127.0.0.1 so the
// /direct bridge (IP literal only) still accepts them.

export const DIRECT_PORT = 21118;

const LOOPBACK = /^(localhost|127\.0\.0\.1|\[?::1\]?)(:\d+)?$/i;
const IPV4 = /^(\d{1,3}\.){3}\d{1,3}$/;

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

export function normalizeDirectTarget(id: string): string {
  let t = id.trim();
  const loopback = LOOPBACK.exec(t);
  if (loopback) {
    t = "127.0.0.1" + (loopback[2] || "");
  }
  if (t.startsWith("[")) {
    return t.indexOf("]:") > 0 ? t : t + ":" + DIRECT_PORT;
  }
  return t.indexOf(":") > 0 ? t : t + ":" + DIRECT_PORT;
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

