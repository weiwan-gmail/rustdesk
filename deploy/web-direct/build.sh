#!/usr/bin/env bash
# Build the rustdesk-web-direct binary (direct-mode web client embedded).
#
#   ./build.sh            # binary for the current platform
#   ./build.sh --all      # linux/amd64, linux/arm64, windows/amd64, darwin/*
#
# It builds the v1 web client from the pinned commit with the shared
# privatization patch (../web/patches/0001) plus the direct-connect patch
# (patches/0002), then embeds it into the Go binary.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEB_DIR="$HERE/../web"
WEB_CLIENT_COMMIT="${WEB_CLIENT_COMMIT:-96f41fcc02dd076bff12053430b90ad2fc43a283}"
REPO_URL="${REPO_URL:-https://github.com/rustdesk/rustdesk}"
BASE_HREF="${BASE_HREF:-/}"
WORK_DIR="${WORK_DIR:-$HERE/.build}"
FLUTTER_VERSION="3.19.6"

SRC="$WORK_DIR/src"
WEB="$SRC/flutter/web"
STATIC="$HERE/server/static"

# --- 1. source tree at the pinned commit -------------------------------------
if [ ! -d "$SRC/flutter/web/js" ]; then
  mkdir -p "$WORK_DIR"
  if git -C "$HERE" rev-parse --git-dir >/dev/null 2>&1 && \
     git -C "$HERE" cat-file -e "$WEB_CLIENT_COMMIT^{commit}" 2>/dev/null; then
    echo ">> using git worktree at $WEB_CLIENT_COMMIT"
    git -C "$HERE" worktree add --detach "$SRC" "$WEB_CLIENT_COMMIT"
  else
    echo ">> downloading $REPO_URL tarball at $WEB_CLIENT_COMMIT"
    mkdir -p "$SRC"
    curl -sL "$REPO_URL/archive/$WEB_CLIENT_COMMIT.tar.gz" | \
      tar xz -C "$SRC" --strip-components=1
  fi
fi

# --- 2. patches: shared privatization + direct connect ------------------------
if ! grep -q RUSTDESK_CONFIG "$WEB/js/src/connection.ts"; then
  echo ">> applying 0001-private-web-client.patch"
  git -C "$SRC" apply "$WEB_DIR/patches/0001-private-web-client.patch" 2>/dev/null \
    || (cd "$SRC" && patch -p1 < "$WEB_DIR/patches/0001-private-web-client.patch")
fi
if ! grep -q _startDirect "$WEB/js/src/connection.ts"; then
  echo ">> applying 0002-direct-connect.patch"
  git -C "$SRC" apply "$HERE/patches/0002-direct-connect.patch" 2>/dev/null \
    || (cd "$SRC" && patch -p1 < "$HERE/patches/0002-direct-connect.patch")
fi
cp "$WEB_DIR/config.js" "$WEB/config.js"

# --- 3. codec bundle (shared with deploy/web) --------------------------------
"$WEB_DIR/fetch-codecs.sh" "$WEB"

# --- 4. JS protocol stack -----------------------------------------------------
if [ "${SKIP_JS:-0}" != "1" ] || [ ! -f "$WEB/js/dist/index.js" ]; then
  echo ">> building JS protocol stack"
  if ! command -v python >/dev/null 2>&1 && command -v python3 >/dev/null 2>&1; then
    mkdir -p "$WORK_DIR/bin"
    ln -sfn "$(command -v python3)" "$WORK_DIR/bin/python"
    export PATH="$WORK_DIR/bin:$PATH"
  fi
  if ! command -v protoc >/dev/null; then
    echo "!! protoc is required (install protobuf-compiler)"
    exit 1
  fi
  # pin-js-deps.sh writes exact versions into package.json (Yarn 1 ignores
  # glob resolutions). npm + overrides: pin typescript 6.0.3 (not 7.x) and a
  # current @types/node that 6.0.3 can parse. libsodium-wrappers@0.7.16
  # ESM-imports ./libsodium.mjs that vite 2.8 cannot resolve.
  "$WEB_DIR/pin-js-deps.sh" "$WEB/js/package.json"
  (cd "$WEB/js" &&
    rm -f yarn.lock package-lock.json &&
    npm install &&
    npm install --no-save --no-package-lock \
      typescript@6.0.3 @types/node@26.3.0 libsodium@0.7.13 libsodium-wrappers@0.7.13 &&
    node -e "
      const tsV = require('typescript/package.json').version;
      const nodeV = require('@types/node/package.json').version;
      const wrapV = require('libsodium-wrappers/package.json').version;
      const sodV = require('libsodium/package.json').version;
      console.log('>> typescript', tsV, '@types/node', nodeV, 'libsodium', sodV, 'wrappers', wrapV);
      if (tsV !== '6.0.3') {
        console.error('!! expected typescript 6.0.3, got ' + tsV);
        process.exit(1);
      }
      if (!String(nodeV).startsWith('26.')) {
        console.error('!! expected @types/node 26.x, got ' + nodeV);
        process.exit(1);
      }
      if (wrapV !== '0.7.13' || sodV !== '0.7.13') {
        console.error('!! expected libsodium 0.7.13, got', sodV, wrapV);
        process.exit(1);
      }
    " &&
    npm run build)
fi

# --- 5. flutter build web -----------------------------------------------------
FLUTTER_BIN="${FLUTTER_ROOT:+$FLUTTER_ROOT/bin/}flutter"
if ! "$FLUTTER_BIN" --version 2>/dev/null | grep -q "Flutter $FLUTTER_VERSION"; then
  echo "!! Flutter $FLUTTER_VERSION is required on PATH (or set FLUTTER_ROOT)"
  exit 1
fi
echo ">> flutter build web --base-href $BASE_HREF"
(cd "$SRC/flutter" && "$FLUTTER_BIN" build web --release --base-href "$BASE_HREF")

# --- 6. stage into the Go binary's static dir + build -------------------------
echo ">> staging static assets"
cp -r "$SRC/flutter/build/web/." "$STATIC/"
cp "$WEB_DIR/config.js" "$STATIC/config.js"

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
