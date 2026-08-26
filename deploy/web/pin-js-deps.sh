#!/usr/bin/env bash
# Pin JS deps so the 2024-era vite 2.8 stack can build on a 2026 toolchain.
# TypeScript is pinned to 6.0.3 (last JS compiler; do not take 7.x here).
# Yarn 1 ignores the "**/@types/node" glob resolution used in the privatization
# patch, so this script writes a direct pin as well. See NOTES.md.
set -euo pipefail

pkg="${1:?usage: pin-js-deps.sh <package.json>}"

python3 - "$pkg" <<'PY'
import json
import sys
from pathlib import Path

p = Path(sys.argv[1])
data = json.loads(p.read_text())
deps = data.setdefault("dependencies", {})
deps["libsodium"] = "0.7.13"
deps["libsodium-wrappers"] = "0.7.13"
dev = data.setdefault("devDependencies", {})
dev["@types/node"] = "26.3.0"
dev["typescript"] = "6.0.3"
res = data.setdefault("resolutions", {})
res["@types/node"] = "26.3.0"
res["**/@types/node"] = "26.3.0"
res["typescript"] = "6.0.3"
res["libsodium"] = "0.7.13"
res["libsodium-wrappers"] = "0.7.13"
overrides = data.setdefault("overrides", {})
overrides["@types/node"] = "26.3.0"
overrides["typescript"] = "6.0.3"
overrides["libsodium"] = "0.7.13"
overrides["libsodium-wrappers"] = "0.7.13"
p.write_text(json.dumps(data, indent=2) + "\n")
print(f">> pinned JS deps in {p}")
PY
