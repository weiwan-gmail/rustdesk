#!/usr/bin/env bash
# Smoke-test the headless API daemon (expects server already running).
set -euo pipefail
BASE="${BASE:-http://127.0.0.1:21120}"
TOKEN="${TOKEN:-}"
AUTH=()
if [[ -n "$TOKEN" ]]; then AUTH=(-H "Authorization: Bearer $TOKEN"); fi

curl -fsS "${AUTH[@]}" "$BASE/api/v1/health" | grep -q '"ok":true'
echo "health ok"

curl -fsS "${AUTH[@]}" -H 'Content-Type: application/json' \
  -d '{"peer_id":"000000000","password":"x"}' \
  "$BASE/api/v1/sessions" > /tmp/rd-session.json
SID=$(python3 -c 'import json; print(json.load(open("/tmp/rd-session.json"))["id"])')
echo "session $SID"

curl -fsS "${AUTH[@]}" -H 'Content-Type: application/json' \
  -d '{"action":"keyboard_type","text":"hi"}' \
  "$BASE/api/v1/sessions/$SID/input/action" | grep -q '"ok":true'
echo "input ok"

curl -fsS "${AUTH[@]}" -H 'Content-Type: application/json' \
  -d '{"text":"clip-test"}' \
  "$BASE/api/v1/sessions/$SID/clipboard" | grep -q '"ok":true'
curl -fsS "${AUTH[@]}" "$BASE/api/v1/sessions/$SID/clipboard" | grep -q 'clip-test'
echo "clipboard ok"

# OS login endpoint (no remote Windows required; empty creds should 400)
code=$(curl -sS -o /tmp/rd-oslogin.json -w '%{http_code}' "${AUTH[@]}" \
  -H 'Content-Type: application/json' \
  -d '{"username":"","password":""}' \
  "$BASE/api/v1/sessions/$SID/os-login" || true)
[[ "$code" == "400" ]] || { echo "expected os-login 400 got $code"; exit 1; }
echo "os-login empty rejected"

# Credentials API (empty store is fine)
curl -fsS "${AUTH[@]}" "$BASE/api/v1/credentials" >/tmp/rd-creds.json
curl -fsS "${AUTH[@]}" -X POST "$BASE/api/v1/credentials/reload" | grep -q '"ok":true'
echo "credentials ok"

# Session info exposes os_login_status
curl -fsS "${AUTH[@]}" "$BASE/api/v1/sessions/$SID" | grep -q 'os_login_status'
echo "os_login_status field ok"

curl -fsS -o /dev/null -w '' "${AUTH[@]}" -X DELETE "$BASE/api/v1/sessions/$SID"
echo "disconnect ok"
