#!/bin/sh
set -e

# Runtime default server for the web client. Empty = same origin (this
# container proxies /ws/id and /ws/relay), which is the right default both
# for the compose stack and for standalone runs behind a gateway.
: "${RUSTDESK_SERVER:=}"

if [ "${CONTROL_ROOM:-}" = "1" ]; then
  cat > /srv/config.js <<EOF
window.RUSTDESK_CONFIG = {server: "${RUSTDESK_SERVER}", wsIdPath: "/ws/id", wsRelayPath: "/ws/relay", control: true, controlPath: "/control", controlBar: true};
EOF
  cat > /etc/caddy/control.handle <<'EOF'
	handle /control* {
		reverse_proxy 127.0.0.1:8099 {
			header_up Host {host}
			header_up X-Real-IP {remote_host}
		}
	}
EOF
  AUTO=""
  if [ "${CONTROL_ROOM_AUTO_APPROVE:-}" = "1" ]; then
    AUTO="--auto-approve"
  fi
  rustdesk-web-controlroom --listen 127.0.0.1:8099 $AUTO &
else
  cat > /srv/config.js <<EOF
window.RUSTDESK_CONFIG = {server: "${RUSTDESK_SERVER}", wsIdPath: "/ws/id", wsRelayPath: "/ws/relay"};
EOF
  echo "# control room disabled" > /etc/caddy/control.handle
fi

exec "$@"
