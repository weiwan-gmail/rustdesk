#!/bin/sh
set -e

# Runtime default server for the web client. Empty = same origin (this
# container proxies /ws/id and /ws/relay), which is the right default both
# for the compose stack and for standalone runs behind a gateway.
: "${RUSTDESK_SERVER:=}"
cat > /srv/config.js <<EOF
window.RUSTDESK_CONFIG = {server: "${RUSTDESK_SERVER}", wsIdPath: "/ws/id", wsRelayPath: "/ws/relay"};
EOF

exec "$@"
