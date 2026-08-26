#!/usr/bin/env bash
# Pin JS deps so the 2024-era tsc (4.4) / vite (2.8) can build on a 2026 toolchain.
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
dev["@types/node"] = "16.18.68"
res = data.setdefault("resolutions", {})
res["@types/node"] = "16.18.68"
res["**/@types/node"] = "16.18.68"
res["libsodium"] = "0.7.13"
res["libsodium-wrappers"] = "0.7.13"
overrides = data.setdefault("overrides", {})
overrides["@types/node"] = "16.18.68"
overrides["libsodium"] = "0.7.13"
overrides["libsodium-wrappers"] = "0.7.13"
p.write_text(json.dumps(data, indent=2) + "\n")
print(f">> pinned JS deps in {p}")
PY
