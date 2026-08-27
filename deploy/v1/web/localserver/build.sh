#!/usr/bin/env bash
# Build the self-contained rustdesk-web binary (web client embedded).
#
#   ./build.sh            # binary for the current platform
#   ./build.sh --all      # linux/amd64, linux/arm64, windows/amd64, darwin/amd64, darwin/arm64
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST="$HERE/../dist"

if [ ! -f "$DIST/index.html" ]; then
  echo ">> dist/ not found, building the web client first"
  "$HERE/../build-web-client.sh"
fi

echo ">> staging static assets"
cp -r "$DIST/." "$HERE/static/"
# Flutter copies web/.gitignore into the build output; restore the placeholder
# ignore so staged assets stay untracked.
cat > "$HERE/static/.gitignore" <<'EOF'
# everything here is build output except this placeholder pair
*
!.gitignore
!index.html
EOF

build() { # <goos> <goarch> <suffix>
  local out="$HERE/rustdesk-web$3"
  echo ">> GOOS=$1 GOARCH=$2 -> $out"
  (cd "$HERE" && GOOS=$1 GOARCH=$2 go build -trimpath -ldflags "-s -w" -o "$out" .)
}

if [ "${1:-}" = "--all" ]; then
  build linux amd64 ""
  build linux arm64 "-linux-arm64"
  build windows amd64 ".exe"
  build darwin amd64 "-macos-amd64"
  build darwin arm64 "-macos-arm64"
else
  build "$(go env GOOS)" "$(go env GOARCH)" ""
fi
echo ">> done"
