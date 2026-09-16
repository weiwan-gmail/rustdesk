#!/usr/bin/env bash
# Stage rustdesk-web-v2 / rustdesk-web-v2-direct into
# deploy/web-helpers/<os-arch>/ for Linux and Windows desktop packages.
#
#   ./deploy/build-web-helpers.sh
#
# v1 is retired (archive: deploy/v1_backup). This script must not build that
# tree. Helpers come from deploy/v2 only.
#
# This script does not compile the v2 client (that is a separate v2 build).
# It writes a marker so CI can still upload the web-helpers artifact; desktop
# packaging skips helpers when the per-triple dirs are empty.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/deploy/web-helpers"
V1_ARCHIVE="$ROOT/deploy/v1_backup"

if [ -d "$ROOT/deploy/v1" ]; then
  echo "!! deploy/v1 must not exist; v1 lives only at deploy/v1_backup" >&2
  exit 1
fi

if [ ! -d "$V1_ARCHIVE" ]; then
  echo "!! expected retired archive at $V1_ARCHIVE" >&2
  exit 1
fi

mkdir -p "$OUT"
cat > "$OUT/README.md" <<'EOF'
# deploy/web-helpers

v1 web helpers are **retired**. Do not rebuild them from `deploy/v1_backup`.

Build and run the current web client from:

- `deploy/v2/web` (server mode, `rustdesk-web-v2`)
- `deploy/v2/web-direct` (direct IP mode, `rustdesk-web-v2-direct`)

Desktop packages skip bundling helpers until a v2 packaging job stages
binaries into the per-triple directories below.
EOF

echo ">> v1 is retired (archive: $V1_ARCHIVE)"
echo ">> current web client: deploy/v2/web and deploy/v2/web-direct"
echo ">> wrote $OUT/README.md (no v1 binaries staged)"
find "$OUT" -type f | sort
