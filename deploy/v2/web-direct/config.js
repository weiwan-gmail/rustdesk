// Runtime configuration for rustdesk-web-direct.
// `direct: true` opts this delivery into IP → /direct (server-mode omits it).
//
// control / controlPath / controlBar: optional exclusive control room. Default
// off. rustdesk-web-v2-direct --control injects control: true at runtime.
window.RUSTDESK_CONFIG = {
  server: "",
  wsIdPath: "/ws/id",
  wsRelayPath: "/ws/relay",
  direct: true,
};
