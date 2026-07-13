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

GUI mode is unchanged when no API flags are passed. **`--api-server` / `--api-connect` / `--api-config` / `--headless-connect` always run as an independent process** and never hand off to or activate an existing RustDesk GUI window.

Do **not** reuse the existing `--config` flag (custom-server license string) or `--import-config` (native Config TOML). Use **`--api-config`** for headless controller options.

## One-shot connect + Windows OS login

Connect outbound and (optionally) auto-type Windows login/lock-screen credentials after the session is up:

```bash
# CLI
rustdesk --api-connect 192.168.1.10 \
  --password rdpass \
  --os-username Administrator \
  --os-password 'WinPass123' \
  --bind 127.0.0.1:21120 \
  --token secret

# Or point a Windows shortcut at a config file (with connect block):
rustdesk --api-config D:\rd-api.json
```

See `examples/api-config.json.example` and `examples/credentials.csv.example`.

### Credential priority

1. Request body / CLI (`--os-username`, `--os-password`, `--password`)
2. `--api-config` fields / `connect` block
3. `--credentials-csv` lookup by `peer_id` or IP
4. No auto OS login

CSV columns: `peer_id,ip,os_username,os_password,rustdesk_password` (empty columns allowed). Missing file is not an error.

### OS login sequence

After connect (when `auto_os_login` is true and OS creds exist): wait `os_login_delay_ms` (default 2500), optionally click-activate the login UI, then type `username` → `Tab` → `password` → `Enter`. Session JSON exposes `os_login_status`: `idle|pending|running|done|failed`.

Manual trigger: `POST /api/v1/sessions/{id}/os-login`.

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/health` | Liveness |
| POST | `/api/v1/sessions` | Connect `{ "peer_id", "password?", "relay?", "os_username?", "os_password?", "auto_os_login?", "os_login_delay_ms?" }` |
| GET | `/api/v1/sessions` | List sessions (includes `os_login_status`) |
| GET | `/api/v1/sessions/{id}` | Session status |
| DELETE | `/api/v1/sessions/{id}` | Disconnect |
| POST | `/api/v1/sessions/{id}/login` | Submit RustDesk password / 2FA |
| POST | `/api/v1/sessions/{id}/os-login` | Type Windows OS username/password once |
| GET | `/api/v1/credentials?peer_id=` | Lookup CSV row (no plaintext passwords; `has_os_password` / username only) |
| POST | `/api/v1/credentials/reload` | Hot-reload CSV |
| GET | `/api/v1/sessions/{id}/screen/latest` | Latest frame (`format=jpeg` or `png`, `encoding=raw` or `json`) |
| WS | `/api/v1/sessions/{id}/screen/stream?fps=2` | Low-FPS frame push (JSON + base64) |
| POST | `/api/v1/sessions/{id}/input/action` | Mouse / keyboard action |
| GET/POST | `/api/v1/sessions/{id}/clipboard` | Read / write remote clipboard text |
| POST | `/api/v1/sessions/{id}/clipboard/copy` | Send Ctrl+C then return clipboard |

Auth: when `--token` is set, send `Authorization: Bearer <token>`.

Coordinates for mouse actions are **remote pixel** coordinates matching the screenshot size.

### Connect with OS login

```json
{
  "peer_id": "406699216",
  "password": "optional-rustdesk-password",
  "os_username": "Administrator",
  "os_password": "WinPass123",
  "auto_os_login": true,
  "os_login_delay_ms": 2500
}
```

### Manual OS login

```json
{
  "username": "Administrator",
  "password": "WinPass123",
  "activate": true,
  "delay_ms": 0,
  "username_first": true
}
```

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

## `--api-config` schema

JSON (default) or TOML (`.toml`). CLI flags override file fields.

| Field | Meaning |
|-------|---------|
| `bind` | Listen address (default `127.0.0.1:21120`) |
| `token` | Bearer token (empty = no auth) |
| `credentials_csv` | Path to credentials CSV |
| `relay` | Force relay |
| `os_login_delay_ms` | Delay after connect before typing |
| `auto_os_login` | Auto-run OS login when creds present |
| `connect` | If present, start oneshot connect (same as `--api-connect`) |
| `connect.peer_id` / `password` / `os_username` / `os_password` | Oneshot peer + passwords |

## Notes

- This is an **outbound controller** (viewer). Remotes still run `rustdesk --server`.
- Frames come from the software pixel-buffer decode path (`HeadlessHandler::on_rgba`). Prefer software codecs when using the API.
- Prefer binding to `127.0.0.1` unless you intentionally expose the daemon and set a token.
- SQLite/Postgres credential backends are not implemented yet; use [`CredentialStore`](../src/api_server/credentials.rs) for future backends.
