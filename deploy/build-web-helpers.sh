#!/usr/bin/env bash
# Build rustdesk-web and rustdesk-web-direct for the Linux/Windows package
# triples and stage them under deploy/web-helpers/<os-arch>/.
#
#   ./deploy/build-web-helpers.sh
#
# Requires: Go 1.22+, Node/npm, Flutter 3.19.6 on PATH (or FLUTTER_ROOT).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/deploy/web-helpers"
WEB="$ROOT/deploy/web"
DIRECT="$ROOT/deploy/web-direct"

echo ">> building rustdesk-web (server mode)"
"$WEB/build-web-client.sh"
"$WEB/localserver/build.sh" --all

echo ">> building rustdesk-web-direct"
"$DIRECT/build.sh" --all

# Extra Windows arm64 Go cross-compile (not in the stock --all lists).
echo ">> extra windows/arm64 binaries"
(cd "$WEB/localserver" && GOOS=windows GOARCH=arm64 go build -trimpath -ldflags "-s -w" -o rustdesk-web-windows-arm64.exe .)
(cd "$DIRECT/server" && GOOS=windows GOARCH=arm64 go build -trimpath -ldflags "-s -w" -o "$DIRECT/rustdesk-web-direct-windows-arm64.exe" .)

stage() { # <src> <dest-dir> <dest-name>
  mkdir -p "$2"
  cp -f "$1" "$2/$3"
  chmod +x "$2/$3" 2>/dev/null || true
  echo "   $1 -> $2/$3"
}

rm -rf "$OUT/linux-amd64" "$OUT/linux-arm64" "$OUT/windows-amd64" "$OUT/windows-arm64"

stage "$WEB/localserver/rustdesk-web"                "$OUT/linux-amd64"    rustdesk-web
stage "$WEB/localserver/rustdesk-web-linux-arm64"    "$OUT/linux-arm64"    rustdesk-web
stage "$WEB/localserver/rustdesk-web.exe"            "$OUT/windows-amd64"  rustdesk-web.exe
stage "$WEB/localserver/rustdesk-web-windows-arm64.exe" "$OUT/windows-arm64" rustdesk-web.exe

stage "$DIRECT/rustdesk-web-direct"                  "$OUT/linux-amd64"    rustdesk-web-direct
stage "$DIRECT/rustdesk-web-direct-linux-arm64"      "$OUT/linux-arm64"    rustdesk-web-direct
stage "$DIRECT/rustdesk-web-direct.exe"              "$OUT/windows-amd64"  rustdesk-web-direct.exe
stage "$DIRECT/rustdesk-web-direct-windows-arm64.exe" "$OUT/windows-arm64" rustdesk-web-direct.exe

echo ">> staged under $OUT"
find "$OUT" -type f | sort
