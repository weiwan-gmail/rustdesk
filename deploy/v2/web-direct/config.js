// Runtime configuration for rustdesk-web-direct.
// `direct: true` opts this delivery into IP → /direct (server-mode omits it).
//
// defaultTarget: optional first-visit Remote ID prefill when last_remote_id is
// empty. When unset, the page uses location.hostname (localhost / ::1 →
// 127.0.0.1). rustdesk-web-v2-direct --default-target injects this at runtime
// and wins over the page host. Does not auto-connect.
//
// control / controlPath / controlBar: optional exclusive control room. Default
// off. rustdesk-web-v2-direct --control injects control: true at runtime.
//
// videoAckRequired: optional LoginRequest.video_ack_required. Unset keeps the
// page default true. rustdesk-web-v2-direct --video-ack=false injects false
// at runtime (H1 lab). Per-tab override: ?video_ack=0 on the page URL.
window.RUSTDESK_CONFIG = {
  server: "",
  wsIdPath: "/ws/id",
  wsRelayPath: "/ws/relay",
  direct: true,
};
