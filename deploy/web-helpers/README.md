# deploy/web-helpers

v1 web helpers are **retired**. Do not rebuild them from `deploy/v1_backup`.

Build and run the current web client from:

- `deploy/v2/web` (server mode, `rustdesk-web-v2`)
- `deploy/v2/web-direct` (direct IP mode, `rustdesk-web-v2-direct`)

Desktop packages skip bundling helpers until a v2 packaging job stages
binaries into the per-triple directories below.
