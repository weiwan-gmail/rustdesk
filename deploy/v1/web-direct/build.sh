#!/usr/bin/env bash
# Build the rustdesk-web-direct binary (direct-mode web client embedded).
#
#   ./build.sh            # binary for the current platform
#   ./build.sh --all      # linux/amd64, linux/arm64, windows/amd64, darwin/*
#
# Builds from the shared vendored tree at ../src. Direct mode is enabled by
# writing `direct: true` into the embedded config.js (server-mode omits it).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_HREF="${BASE_HREF:-/}"
FLUTTER_VERSION="3.19.6"

SRC="$HERE/../src"
WEB="$SRC/flutter/web"
STATIC="$HERE/server/static"

if [ ! -d "$WEB/js" ]; then
  echo "!! vendored source missing at $SRC (expected flutter/web/js)"
  exit 1
fi

# --- 1. codec bundle ----------------------------------------------------------
"$SRC/fetch-codecs.sh" "$WEB"

# --- 2. JS protocol stack -----------------------------------------------------
if [ "${SKIP_JS:-0}" != "1" ] || [ ! -f "$WEB/js/dist/index.js" ]; then
  echo ">> building JS protocol stack"
  if ! command -v python >/dev/null 2>&1 && command -v python3 >/dev/null 2>&1; then
    mkdir -p "$HERE/.build/bin"
    ln -sfn "$(command -v python3)" "$HERE/.build/bin/python"
    export PATH="$HERE/.build/bin:$PATH"
  fi
  if ! command -v protoc >/dev/null; then
    echo "!! protoc is required (install protobuf-compiler)"
    exit 1
  fi
  (cd "$WEB/js" &&
    rm -f yarn.lock package-lock.json &&
    npm install &&
    npm install --no-save --no-package-lock \
      typescript@6.0.3 @types/node@26.3.0 vite@7.3.6 \
      libsodium@0.7.13 libsodium-wrappers@0.7.13 &&
    node -e "
      const tsV = require('typescript/package.json').version;
      const nodeV = require('@types/node/package.json').version;
      const viteV = require('vite/package.json').version;
      const wrapV = require('libsodium-wrappers/package.json').version;
      const sodV = require('libsodium/package.json').version;
      console.log('>> typescript', tsV, '@types/node', nodeV, 'vite', viteV, 'libsodium', sodV, 'wrappers', wrapV);
      if (tsV !== '6.0.3') {
        console.error('!! expected typescript 6.0.3, got ' + tsV);
        process.exit(1);
      }
      if (!String(nodeV).startsWith('26.')) {
        console.error('!! expected @types/node 26.x, got ' + nodeV);
        process.exit(1);
      }
      if (viteV !== '7.3.6') {
        console.error('!! expected vite 7.3.6, got ' + viteV);
        process.exit(1);
      }
      if (wrapV !== '0.7.13' || sodV !== '0.7.13') {
        console.error('!! expected libsodium 0.7.13, got', sodV, wrapV);
        process.exit(1);
      }
    " &&
    npm run build &&
    if [ ! -f dist/index.js ] || [ ! -f dist/vendor.js ]; then
      echo '!! expected dist/index.js and dist/vendor.js (Flutter index.html hardcodes both)'
      ls -la dist || true
      exit 1
    fi)
fi

# --- 3. flutter build web -----------------------------------------------------
FLUTTER_BIN="${FLUTTER_ROOT:+$FLUTTER_ROOT/bin/}flutter"
if ! "$FLUTTER_BIN" --version 2>/dev/null | grep -q "Flutter $FLUTTER_VERSION"; then
  echo "!! Flutter $FLUTTER_VERSION is required on PATH (or set FLUTTER_ROOT)"
  exit 1
fi
echo ">> flutter build web --base-href $BASE_HREF"
(cd "$SRC/flutter" && "$FLUTTER_BIN" build web --release --base-href "$BASE_HREF")

# --- 4. stage into the Go binary's static dir + build -------------------------
echo ">> staging static assets"
cp -r "$SRC/flutter/build/web/." "$STATIC/"
cp "$HERE/config.js" "$STATIC/config.js"

build_bin() { # <goos> <goarch> <suffix>
  local out="$HERE/rustdesk-web-direct$3"
  echo ">> GOOS=$1 GOARCH=$2 -> $out"
  (cd "$HERE/server" && GOOS=$1 GOARCH=$2 go build -trimpath -ldflags "-s -w" -o "$out" .)
}

if [ "${1:-}" = "--all" ]; then
  build_bin linux amd64 ""
  build_bin linux arm64 "-linux-arm64"
  build_bin windows amd64 ".exe"
  build_bin darwin amd64 "-macos-amd64"
  build_bin darwin arm64 "-macos-arm64"
else
  build_bin "$(go env GOOS)" "$(go env GOARCH)" ""
fi
echo ">> done: $HERE/rustdesk-web-direct"
