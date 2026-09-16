# deploy/

**Future web-client work uses `deploy/v2` only** (server mode under `v2/web`,
direct IP mode under `v2/web-direct`). Do not start new features, fixes, or
packaging on the v1 tree.

```text
v2/           current web client (builds the repo's flutter/ tree)
v1_backup/    retired v1 archive — keep for history, do not develop against it
web-helpers/  staged desktop-package helpers (v2; v1 is no longer built)
```

Start here:

- Server mode (hbbs/hbbr): [`v2/README.md`](v2/README.md)
- Direct IP mode (no hbbs/hbbr): [`v2/web-direct/README.md`](v2/web-direct/README.md)
- Optional exclusive control among v2 viewers: [`v2/controlroom/README.md`](v2/controlroom/README.md)

`v1_backup/` is a frozen v1.2.4-era snapshot. It is not a supported path.
