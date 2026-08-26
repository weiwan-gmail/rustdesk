#!/usr/bin/env bash
# Build the RustDesk web client (Flutter Web + JS protocol stack) from the
# last upstream commit where flutter/lib and flutter/web/js were in sync.
#
# Usage:
#   ./build-web-client.sh                 # full build into ./dist
#   BASE_HREF=/rustdesk/ ./build-web-client.sh
#
# Env:
#   WEB_CLIENT_COMMIT  source commit            (default: pinned, see below)
#   REPO_URL           tarball source repo      (default: https://github.com/rustdesk/rustdesk)
#   BASE_HREF          base href of the build   (default: /)
#   WORK_DIR           scratch dir              (default: ./.build)
#   SKIP_JS=1          reuse existing js/dist   (default: off)
#
# Requirements: node + npm, python3, protoc, yarn, ts-proto, and Flutter
# 3.19.6 on PATH (FLUTTER_ROOT also honored). See README.md.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEB_CLIENT_COMMIT="${WEB_CLIENT_COMMIT:-96f41fcc02dd076bff12053430b90ad2fc43a283}"
REPO_URL="${REPO_URL:-https://github.com/rustdesk/rustdesk}"
BASE_HREF="${BASE_HREF:-/}"
WORK_DIR="${WORK_DIR:-$HERE/.build}"
FLUTTER_VERSION="3.19.6"

SRC="$WORK_DIR/src"
WEB="$SRC/flutter/web"
DIST="$HERE/dist"

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

# --- 2. privatization patch ---------------------------------------------------
if ! grep -q RUSTDESK_CONFIG "$WEB/js/src/connection.ts"; then
  echo ">> applying patches/0001-private-web-client.patch"
  git -C "$SRC" apply "$HERE/patches/0001-private-web-client.patch" 2>/dev/null \
    || (cd "$SRC" && patch -p1 < "$HERE/patches/0001-private-web-client.patch")
fi
cp "$HERE/config.js" "$WEB/config.js"

# --- 3. codec bundle ----------------------------------------------------------
"$HERE/fetch-codecs.sh" "$WEB"

# --- 4. JS protocol stack -----------------------------------------------------
if [ "${SKIP_JS:-0}" != "1" ] || [ ! -f "$WEB/js/dist/index.js" ]; then
  echo ">> building JS protocol stack"
  if ! command -v python >/dev/null 2>&1 && command -v python3 >/dev/null 2>&1; then
    mkdir -p "$WORK_DIR/bin"
    ln -sfn "$(command -v python3)" "$WORK_DIR/bin/python"
    export PATH="$WORK_DIR/bin:$PATH"
  fi
  command -v yarn >/dev/null || npm install -g yarn
  if ! command -v protoc >/dev/null; then
    echo "!! protoc is required (install protobuf-compiler)"
    exit 1
  fi
  # The 2024 snapshot ships a Yarn Berry lockfile. Yarn 1 cannot apply
  # package.json resolutions on top of it and may pull an unusable @types/node.
  (cd "$WEB/js" && rm -f yarn.lock && yarn install && \
    if [ -f node_modules/@types/node/ffi.d.ts ]; then
      echo "!! refusing ancient @types/node (ffi.d.ts present)"
      exit 1
    fi && yarn build)
fi

# --- 5. flutter build web -----------------------------------------------------
FLUTTER_BIN="${FLUTTER_ROOT:+$FLUTTER_ROOT/bin/}flutter"
if ! "$FLUTTER_BIN" --version 2>/dev/null | grep -q "Flutter $FLUTTER_VERSION"; then
  echo "!! Flutter $FLUTTER_VERSION is required on PATH (or set FLUTTER_ROOT):"
  echo "   https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_${FLUTTER_VERSION}-stable.tar.xz"
  exit 1
fi
echo ">> flutter build web --base-href $BASE_HREF"
(cd "$SRC/flutter" && "$FLUTTER_BIN" build web --release --base-href "$BASE_HREF")

# --- 6. collect ---------------------------------------------------------------
rm -rf "$DIST"
mkdir -p "$DIST"
cp -r "$SRC/flutter/build/web/." "$DIST/"
cp "$HERE/config.js" "$DIST/config.js"
echo ">> web client built: $DIST"
