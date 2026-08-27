# deploy/v1

Frozen v1 web client (Flutter 3.19.6 / v1.2.4-era UI) for private deployment.

```text
src/          vendored source (commit 96f41fcc + in-tree privatization/direct)
web/          server-mode delivery (static page + WS proxy to hbbs/hbbr)
web-direct/   direct-mode delivery (IP → /direct, no hbbs/hbbr)
```

`src/` is shared. Direct mode is a runtime flag (`RUSTDESK_CONFIG.direct`), not a second source tree. Server-mode `config.js` omits the flag, so an IP typed as an ID still goes to rendezvous (same as the old 0001-only build).

v2 lives at `deploy/v2`: it builds the repository's current `flutter/` tree
(web root at `flutter/web/`) instead of a vendored snapshot.
