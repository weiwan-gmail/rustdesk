# deploy/v2

V2 web client: built from the repository's **current** `flutter/` tree
(Flutter 3.24.5, the same code as the desktop client), unlike v1 which is a
frozen v1.2.4-era snapshot vendored at `deploy/v1/src`.

```text
../flutter/web/     v2 web root (index.html, js/ protocol stack) - tracked in the main tree
web/                server-mode delivery (static page + WS proxy to hbbs/hbbr)
fetch-codecs.sh     codec bundle fetcher (ogv.js / yuv-canvas / libopus)
```

v1 is kept untouched under `deploy/v1` for comparison.

## What v2 is

- **UI**: the current Flutter UI (`flutter/lib`), built with
  `flutter build web`. The web entry is `WebHomePage` (connection page) and
  the desktop `RemotePage` for sessions — the same widgets as the desktop
  client, so the look & feel tracks the current GUI.
- **Protocol stack**: `flutter/web/js/` — a TypeScript stack (WebSocket +
  libsodium secretbox + ts-proto generated from the current
  `libs/hbb_common/protos`) implementing the `setByName`/`getByName` bridge
  that `flutter/lib/web/bridge.dart` expects. It talks to hbbs/hbbr exactly
  like v1 did (rendezvous -> relay -> encrypted session).

### Supported

- Outgoing remote desktop (view, mouse/keyboard input, clipboard, chat,
  audio, display switch, quality options)

### Not yet

- File transfer / terminal pages connect but perform no actions yet
- Address book sync, account login (OIDC), LAN discovery, voice call
- Incoming connections (a browser cannot be controlled)

## Build & run

Same delivery style as v1 (`deploy/v1/web`), but the build context is the
repository root:

```bash
# local build (needs Flutter 3.24.5, node, python3, protoc)
cd deploy/v2/web && ./build-web-client.sh        # output: deploy/v2/web/dist

# all-in-one image
docker build -f deploy/v2/web/Dockerfile.web -t rustdesk-web-v2 .
docker run -p 8080:80 -e SITE_ADDRESS=http://:80 rustdesk-web-v2

# compose (hbbs + hbbr + web)
cd deploy/v2/web && cp .env.example .env && docker compose up -d
```

Then open `http://<server-ip>:8080`. See `deploy/v1/web/README.md` for the
full deployment guide — the runtime contract (`config.js`, `/ws/id`,
`/ws/relay`, `SITE_ADDRESS`, `BASE_HREF`) is identical.
