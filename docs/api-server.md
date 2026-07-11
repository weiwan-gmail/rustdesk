# RustDesk Headless API Server

Build with the `api-server` feature and start a headless controller daemon:

```bash
cargo build --features api-server --release
./target/release/rustdesk --api-server --bind 127.0.0.1:21120
# optional auth:
./target/release/rustdesk --api-server --bind 127.0.0.1:21120 --token secret
```

On Linux without vcpkg, you can also build with system codecs:

```bash
cargo build --features "api-server,linux-pkg-config"
```

GUI mode is unchanged when `--api-server` is not passed.

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/health` | Liveness |
| POST | `/api/v1/sessions` | Connect to a peer `{ "peer_id", "password?", "relay?" }` |
| GET | `/api/v1/sessions` | List sessions |
| GET | `/api/v1/sessions/{id}` | Session status |
| DELETE | `/api/v1/sessions/{id}` | Disconnect |
| POST | `/api/v1/sessions/{id}/login` | Submit password / 2FA |
| GET | `/api/v1/sessions/{id}/screen/latest` | Latest frame (`format=jpeg` or `png`, `encoding=raw` or `json`) |
| WS | `/api/v1/sessions/{id}/screen/stream?fps=2` | Low-FPS frame push (JSON + base64) |
| POST | `/api/v1/sessions/{id}/input/action` | Mouse / keyboard action |
| GET/POST | `/api/v1/sessions/{id}/clipboard` | Read / write remote clipboard text |
| POST | `/api/v1/sessions/{id}/clipboard/copy` | Send Ctrl+C then return clipboard |

Auth: when `--token` is set, send `Authorization: Bearer <token>`.

Coordinates for mouse actions are **remote pixel** coordinates matching the screenshot size.

### Input examples

```json
{ "action": "mouse_click", "x": 850, "y": 420, "button": "left", "type": "double_click" }
```

```json
{ "action": "keyboard_type", "text": "sudo apt update\n" }
```

```json
{ "action": "keyboard_key", "key": "VK_RETURN", "press": true }
```

## Notes

- This is an **outbound controller** (viewer). Remotes still run `rustdesk --server`.
- Frames come from the software pixel-buffer decode path (`HeadlessHandler::on_rgba`). Prefer software codecs when using the API.
- Prefer binding to `127.0.0.1` unless you intentionally expose the daemon and set a token.
