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

curl -fsS -o /dev/null -w '' "${AUTH[@]}" -X DELETE "$BASE/api/v1/sessions/$SID"
echo "disconnect ok"
