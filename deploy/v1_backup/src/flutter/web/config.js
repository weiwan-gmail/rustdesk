// Runtime configuration for the RustDesk web client.
// This file is served next to index.html and read before the app starts.
//
// server: default RustDesk server written into the client on first load.
//   ""                 -> same origin: the page is served by rustdesk-web or
//                         the bundled Caddy, which proxy /ws/id and /ws/relay.
//   "host"             -> domain: ws(s)://host/ws/id (reverse proxy on 80/443)
//   "1.2.3.4"          -> IP: ws(s)://1.2.3.4:21118 / :21119
//   "host:21116"       -> explicit port: ws(s)://host:21118 / :21119
// Users can always override the server in the client settings page.
//
// wsIdPath / wsRelayPath: WS endpoint paths used for the domain / same-origin
// cases above. Change only if your gateway maps different paths.
window.RUSTDESK_CONFIG = {
  server: "",
  wsIdPath: "/ws/id",
  wsRelayPath: "/ws/relay",
};
