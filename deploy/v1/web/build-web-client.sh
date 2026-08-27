#!/usr/bin/env bash
# Build the RustDesk web client (Flutter Web + JS protocol stack) from the
# vendored v1 source at deploy/v1/src.
#
# Usage:
#   ./build-web-client.sh                 # full build into ./dist
#   BASE_HREF=/rustdesk/ ./build-web-client.sh
#
# Env:
#   BASE_HREF          base href of the build   (default: /)
#   SKIP_JS=1          reuse existing js/dist   (default: off)
#
# Requirements: node + npm, python3, protoc, yarn, ts-proto, and Flutter
# 3.19.6 on PATH (FLUTTER_ROOT also honored). See README.md.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_HREF="${BASE_HREF:-/}"
FLUTTER_VERSION="3.19.6"

SRC="$HERE/../src"
WEB="$SRC/flutter/web"
DIST="$HERE/dist"

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
  # Vendored package.json already pins typescript 6.0.3, vite 7.3.6,
  # libsodium 0.7.13, and @types/node 26.x. Re-assert after npm install.
  (cd "$WEB/js" &&
    rm -f package-lock.json &&
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
  echo "!! Flutter $FLUTTER_VERSION is required on PATH (or set FLUTTER_ROOT):"
  echo "   https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_${FLUTTER_VERSION}-stable.tar.xz"
  exit 1
fi
echo ">> flutter build web --base-href $BASE_HREF"
(cd "$SRC/flutter" && "$FLUTTER_BIN" build web --release --base-href "$BASE_HREF")

# --- 4. collect ---------------------------------------------------------------
rm -rf "$DIST"
mkdir -p "$DIST"
cp -r "$SRC/flutter/build/web/." "$DIST/"
cp "$HERE/config.js" "$DIST/config.js"
echo ">> web client built: $DIST"
