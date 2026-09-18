/// First-visit Remote ID for rustdesk-web-v2-direct (prefill only).
///
/// Saved [lastRemoteId] always wins. Otherwise, only web + direct mode
/// prefill: `defaultTarget` from config, else the page host when that host
/// is already useful as a direct IP (loopback → `127.0.0.1`). Hostnames are
/// left empty; LAN DNS is a follow-up.
String resolveWebDirectRemoteId({
  required bool isWeb,
  required bool direct,
  required String lastRemoteId,
  required String defaultTarget,
  required String locationHost,
}) {
  final saved = lastRemoteId.trim();
  if (saved.isNotEmpty) return saved;
  if (!isWeb || !direct) return '';
  final fromConfig = defaultTarget.trim();
  if (fromConfig.isNotEmpty) return fromConfig;
  return usefulDirectConnectHost(locationHost) ?? '';
}

String? usefulDirectConnectHost(String host) {
  var h = host.trim();
  if (h.isEmpty) return null;
  if (h.startsWith('[') && h.endsWith(']') && h.length > 2) {
    h = h.substring(1, h.length - 1);
  }
  final lower = h.toLowerCase();
  if (lower == 'localhost' || lower == '127.0.0.1' || lower == '::1') {
    return '127.0.0.1';
  }
  if (_ipv4.hasMatch(h)) return h;
  if (h.contains(':')) return h;
  return null;
}

final _ipv4 = RegExp(r'^(\d{1,3}\.){3}\d{1,3}$');
