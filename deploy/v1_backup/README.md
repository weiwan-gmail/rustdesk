# deploy/v1_backup (retired)

**This tree is an archive. Do not develop against it.**

Future web-client work must use **`deploy/v2`** (server mode) and
**`deploy/v2/web-direct`** (direct IP mode). Do not add features, fixes, CI
jobs, or packaging that treat this directory as a live product.

This directory used to be `deploy/v1`. It is kept only so the frozen
v1.2.4-era Flutter Web client (Flutter 3.19.6) can still be inspected. The
contents are otherwise unchanged.

```text
src/          vendored source (commit 96f41fcc + in-tree privatization/direct)
web/          archived server-mode delivery (static page + WS proxy to hbbs/hbbr)
web-direct/   archived direct-mode delivery (IP → /direct, no hbbs/hbbr)
```

Supported path: [`../v2/README.md`](../v2/README.md) and
[`../v2/web-direct/README.md`](../v2/web-direct/README.md).
