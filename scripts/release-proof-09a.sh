#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
PORT="${KANBAN_RELEASE_PORT:-18721}"
BASE_URL="http://127.0.0.1:$PORT"
EVIDENCE_DIR="${KANBAN_RELEASE_EVIDENCE_DIR:-$ROOT/output/release}"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/kanban-release-09a.XXXXXX")"
DB_PATH="$TMP_ROOT/canonical.db"
HOST_LOG="$TMP_ROOT/host.log"
HOST_PID=""
HOST_START=""

cleanup() {
  set +e
  if [[ -n "$HOST_PID" ]] && kill -0 "$HOST_PID" 2>/dev/null; then
    current_start="$(awk '{print $22}' "/proc/$HOST_PID/stat" 2>/dev/null)"
    if [[ "$current_start" == "$HOST_START" ]]; then
      kill -INT -- "-$HOST_PID" 2>/dev/null
      for _ in $(seq 1 80); do
        if ! kill -0 "$HOST_PID" 2>/dev/null; then break; fi
        current_start="$(awk '{print $22}' "/proc/$HOST_PID/stat" 2>/dev/null)"
        [[ "$current_start" == "$HOST_START" ]] || break
        sleep 0.1
      done
      if kill -0 "$HOST_PID" 2>/dev/null; then kill -TERM -- "-$HOST_PID" 2>/dev/null; fi
    fi
  fi
  if [[ -n "$HOST_PID" ]]; then wait "$HOST_PID" 2>/dev/null; fi
  if [[ -f "$HOST_LOG" ]]; then cp -- "$HOST_LOG" "$EVIDENCE_DIR/host.log"; fi
  rm -rf -- "$TMP_ROOT"
}
trap cleanup EXIT

for tool in curl jq pnpm setsid; do
  command -v "$tool" >/dev/null 2>&1 || { echo "error: release 09A requires $tool" >&2; exit 1; }
done
if curl --silent --max-time 0.5 "$BASE_URL/health" >/dev/null 2>&1; then
  echo "error: release port already serves a host: $BASE_URL" >&2
  exit 1
fi

mkdir -p -- "$EVIDENCE_DIR"
for evidence_name in host.json browser.json browser-chromium.json browser-firefox.json host.log release-proof-receipt.json; do
  evidence_path="$EVIDENCE_DIR/$evidence_name"
  if [[ -f "$evidence_path" || -L "$evidence_path" ]]; then
    rm -f -- "$evidence_path"
  fi
done
just --justfile "$ROOT/justfile" web-build
"$LOCK" -- cargo build --locked -p kanban-cli
TARGET_DIR="$("$LOCK" --print-target-dir)"
KANBAN="$TARGET_DIR/debug/kanban"
[[ -x "$KANBAN" ]] || { echo "error: kanban binary missing: $KANBAN" >&2; exit 1; }

setsid env KANBAN_ACTOR=release-09a "$KANBAN" --db "$DB_PATH" serve --host 127.0.0.1 --port "$PORT" --web-dir "$ROOT/apps/web/dist" >"$HOST_LOG" 2>&1 &
HOST_PID="$!"
HOST_START="$(awk '{print $22}' "/proc/$HOST_PID/stat")"
HOST_CMDLINE="$(tr '\0' ' ' < "/proc/$HOST_PID/cmdline")"
[[ "$HOST_CMDLINE" == *kanban* && "$HOST_CMDLINE" == *serve* ]] || { echo "error: host PID command line is not kanban serve" >&2; exit 1; }
HOST_PID_BEFORE="$HOST_PID"
HOST_START_BEFORE="$HOST_START"
HOST_CMDLINE_BEFORE="$HOST_CMDLINE"
REAL_HOST_CHECKED=false

curl --fail --silent --show-error --retry 120 --retry-all-errors --retry-delay 1 "$BASE_URL/health" >/dev/null
BOARDS_JSON="$(KANBAN_SERVER_URL="$BASE_URL" "$KANBAN" --json --board default board list)"
if ! jq -e '.data | any(.slug == "default")' <<<"$BOARDS_JSON" >/dev/null; then
  KANBAN_SERVER_URL="$BASE_URL" "$KANBAN" --json --board default board create default --name "Release 09A" >/dev/null
fi
KANBAN_SERVER_URL="$BASE_URL" "$KANBAN" --json --board default task create "Seed release task" --status todo --task-id t_release_seed >/dev/null

HEALTH_JSON="$(curl --fail --silent --show-error "$BASE_URL/health")"
RUNTIME_JSON="$(curl --fail --silent --show-error "$BASE_URL/app/runtime.json")"
MANIFEST_JSON="$(curl --fail --silent --show-error "$BASE_URL/app/manifest.json")"
SEED_JSON="$(KANBAN_SERVER_URL="$BASE_URL" "$KANBAN" --json --board default task show t_release_seed)"
[[ "$(jq -er '.data.title' <<<"$SEED_JSON")" == "Seed release task" ]] || { echo "error: seeded task was not readable before restart" >&2; exit 1; }
DOCTOR_JSON="$(curl --fail --silent --show-error "$BASE_URL/api/v1/maintenance/doctor")"
MIGRATION_BEFORE="$(jq -er '.data.migration_version // .data.user_version' <<<"$DOCTOR_JSON")"
DB_FINGERPRINT_RESTART_BEFORE="$(jq -er '.data.db_fingerprint' <<<"$HEALTH_JSON")"

# Restart the same canonical DB through a fresh process before browser assertions.
kill -INT -- "-$HOST_PID"
for _ in $(seq 1 80); do
  if ! kill -0 "$HOST_PID" 2>/dev/null; then break; fi
  sleep 0.1
done
if kill -0 "$HOST_PID" 2>/dev/null; then
  echo "error: first kanban serve did not stop before restart" >&2
  exit 1
fi
if curl --silent --max-time 0.5 "$BASE_URL/health" >/dev/null 2>&1; then
  echo "error: canonical host port remained open after first process exit" >&2
  exit 1
fi
wait "$HOST_PID" 2>/dev/null

setsid env KANBAN_ACTOR=release-09a "$KANBAN" --db "$DB_PATH" serve --host 127.0.0.1 --port "$PORT" --web-dir "$ROOT/apps/web/dist" >>"$HOST_LOG" 2>&1 &
HOST_PID="$!"
HOST_START="$(awk '{print $22}' "/proc/$HOST_PID/stat")"
HOST_CMDLINE="$(tr '\0' ' ' < "/proc/$HOST_PID/cmdline")"
[[ "$HOST_CMDLINE" == *kanban* && "$HOST_CMDLINE" == *serve* ]] || { echo "error: restarted PID command line is not kanban serve" >&2; exit 1; }
[[ "$HOST_PID" != "$HOST_PID_BEFORE" || "$HOST_START" != "$HOST_START_BEFORE" ]] || { echo "error: restart did not produce a distinct PID identity" >&2; exit 1; }
curl --fail --silent --show-error --retry 120 --retry-all-errors --retry-delay 1 "$BASE_URL/health" >/dev/null
HEALTH_JSON="$(curl --fail --silent --show-error "$BASE_URL/health")"
RUNTIME_JSON="$(curl --fail --silent --show-error "$BASE_URL/app/runtime.json")"
MANIFEST_JSON="$(curl --fail --silent --show-error "$BASE_URL/app/manifest.json")"
DB_FINGERPRINT_RESTART_AFTER="$(jq -er '.data.db_fingerprint' <<<"$HEALTH_JSON")"
[[ "$DB_FINGERPRINT_RESTART_BEFORE" == "$DB_FINGERPRINT_RESTART_AFTER" ]] || {
  echo "error: canonical DB fingerprint changed across host restart (before=$DB_FINGERPRINT_RESTART_BEFORE after=$DB_FINGERPRINT_RESTART_AFTER)" >&2
  exit 1
}
SEED_JSON_AFTER="$(KANBAN_SERVER_URL="$BASE_URL" "$KANBAN" --json --board default task show t_release_seed)"
SEED_TITLE_AFTER="$(jq -er '.data.title' <<<"$SEED_JSON_AFTER")"
SEED_RECOVERED=false
if [[ "$SEED_TITLE_AFTER" == "Seed release task" ]]; then SEED_RECOVERED=true; fi
[[ "$SEED_RECOVERED" == true ]] || { echo "error: seeded task did not survive host restart" >&2; exit 1; }

HOST_NEGATIVE_STATUS="$(curl --silent --show-error --output /dev/null --write-out '%{http_code}' -H 'Host: evil.invalid' "$BASE_URL/app/")"
ORIGIN_NEGATIVE_STATUS="$(curl --silent --show-error --output /dev/null --write-out '%{http_code}' -H 'Origin: https://evil.invalid' "$BASE_URL/app/")"
ORIGIN_NEGATIVE_ACAO="$(curl --silent --show-error --dump-header - -o /dev/null -H 'Origin: https://evil.invalid' "$BASE_URL/app/" | awk 'BEGIN{IGNORECASE=1} /^access-control-allow-origin:/{sub("^[^:]*:[[:space:]]*", ""); gsub("\r", ""); print; exit}')"
ORIGIN_NULL_STATUS="$(curl --silent --show-error --output /dev/null --write-out '%{http_code}' -H 'Origin: null' "$BASE_URL/app/")"
ORIGIN_NULL_ACAO="$(curl --silent --show-error --dump-header - -o /dev/null -H 'Origin: null' "$BASE_URL/app/" | awk 'BEGIN{IGNORECASE=1} /^access-control-allow-origin:/{sub("^[^:]*:[[:space:]]*", ""); gsub("\r", ""); print; exit}')"
CSP_HEADER="$(curl --silent --show-error --dump-header - -o /dev/null "$BASE_URL/app/" | awk 'BEGIN{IGNORECASE=1} /^content-security-policy:/{sub("^[^:]*:[[:space:]]*", ""); gsub("\r", ""); print; exit}')"
HOST_NEGATIVE_CHECKED=false
ORIGIN_NEGATIVE_CHECKED=false
ORIGIN_NULL_REJECTED=false
CSP_CHECKED=false
if [[ "$HOST_NEGATIVE_STATUS" == "400" || "$HOST_NEGATIVE_STATUS" == "403" ]]; then HOST_NEGATIVE_CHECKED=true; fi
if [[ "$ORIGIN_NEGATIVE_STATUS" == "400" || "$ORIGIN_NEGATIVE_STATUS" == "403" ]] && [[ -z "$ORIGIN_NEGATIVE_ACAO" ]]; then ORIGIN_NEGATIVE_CHECKED=true; fi
if [[ "$ORIGIN_NULL_STATUS" == "400" || "$ORIGIN_NULL_STATUS" == "403" ]] && [[ -z "$ORIGIN_NULL_ACAO" ]]; then ORIGIN_NULL_REJECTED=true; fi
if grep -Fq "default-src 'self'" <<<"$CSP_HEADER" && grep -Fq "script-src 'self'" <<<"$CSP_HEADER"; then CSP_CHECKED=true; fi
[[ "$HOST_NEGATIVE_CHECKED" == true ]] || { echo "error: hostile Host was not rejected (status=$HOST_NEGATIVE_STATUS)" >&2; exit 1; }
[[ "$ORIGIN_NEGATIVE_CHECKED" == true ]] || { echo "error: hostile Origin was not rejected without ACAO (status=$ORIGIN_NEGATIVE_STATUS, acao=$ORIGIN_NEGATIVE_ACAO)" >&2; exit 1; }
[[ "$ORIGIN_NULL_REJECTED" == true ]] || { echo "error: Origin null was not rejected without ACAO (status=$ORIGIN_NULL_STATUS, acao=$ORIGIN_NULL_ACAO)" >&2; exit 1; }
[[ "$CSP_CHECKED" == true ]] || { echo "error: app CSP missing self restrictions" >&2; exit 1; }
REAL_HOST_CHECKED=true

tmp_host="$EVIDENCE_DIR/.host.json.$HOST_PID.tmp"
jq -n \
  --arg base_url "$BASE_URL" \
  --argjson pid "$HOST_PID" \
  --argjson pid_before "$HOST_PID_BEFORE" \
  --arg start_before "$HOST_START_BEFORE" \
  --arg start_after "$HOST_START" \
  --arg command_line_before "$HOST_CMDLINE_BEFORE" \
  --arg command_line "$HOST_CMDLINE" \
  --arg health "$(jq -c '.data' <<<"$HEALTH_JSON")" \
  --arg runtime "$RUNTIME_JSON" \
  --arg manifest "$MANIFEST_JSON" \
  --arg seed_title "$SEED_TITLE_AFTER" \
  --arg fingerprint "$DB_FINGERPRINT_RESTART_BEFORE" \
  --arg restart_fingerprint_before "$DB_FINGERPRINT_RESTART_BEFORE" \
  --arg restart_fingerprint_after "$DB_FINGERPRINT_RESTART_AFTER" \
  --argjson migration "$MIGRATION_BEFORE" \
  --argjson host_negative_checked "$HOST_NEGATIVE_CHECKED" \
  --argjson origin_negative_checked "$ORIGIN_NEGATIVE_CHECKED" \
  --argjson origin_null_rejected "$ORIGIN_NULL_REJECTED" \
  --argjson csp_checked "$CSP_CHECKED" \
  --argjson real_host "$REAL_HOST_CHECKED" \
  --argjson seed_recovered "$SEED_RECOVERED" \
  '{schema_version:1,real_host:$real_host,base_url:$base_url,pid:$pid,pid_before:$pid_before,start_before:$start_before,start_after:$start_after,command_line:$command_line,command_line_before:$command_line_before,health:($health|fromjson),runtime:($runtime|fromjson),manifest:($manifest|fromjson),seed_title:$seed_title,seed_recovered:$seed_recovered,db_fingerprint_before:$fingerprint,db_fingerprint_restart_before:$restart_fingerprint_before,db_fingerprint_restart_after:$restart_fingerprint_after,migration_before:$migration,host_negative_checked:$host_negative_checked,origin_negative_checked:$origin_negative_checked,origin_null_rejected:$origin_null_rejected,csp_checked:$csp_checked}' \
  >"$tmp_host"
mv -- "$tmp_host" "$EVIDENCE_DIR/host.json"

KANBAN_RELEASE_PROOF=1 KANBAN_RELEASE_BASE_URL="$BASE_URL" KANBAN_RELEASE_EVIDENCE_DIR="$EVIDENCE_DIR" \
  pnpm --filter '@kanban-tool/web' exec playwright test --project=chromium --project=firefox

CHROMIUM_JSON="$EVIDENCE_DIR/browser-chromium.json"
FIREFOX_JSON="$EVIDENCE_DIR/browser-firefox.json"
[[ -f "$CHROMIUM_JSON" && -f "$FIREFOX_JSON" ]] || { echo "error: browser evidence missing" >&2; exit 1; }
tmp_browser="$EVIDENCE_DIR/.browser.json.$HOST_PID.tmp"
CHROMIUM_REAL_HOST="$(jq -er '.real_host' "$CHROMIUM_JSON")"
FIREFOX_REAL_HOST="$(jq -er '.real_host' "$FIREFOX_JSON")"
REAL_BROWSER_HOST=false
if [[ "$CHROMIUM_REAL_HOST" == true && "$FIREFOX_REAL_HOST" == true ]]; then REAL_BROWSER_HOST=true; fi
[[ "$REAL_BROWSER_HOST" == true ]] || { echo "error: browser lanes did not prove real host" >&2; exit 1; }
jq -n \
  --arg base_url "$BASE_URL" \
  --arg chromium "$(jq -er '.chromium' "$CHROMIUM_JSON")" \
  --arg firefox "$(jq -er '.firefox' "$FIREFOX_JSON")" \
  --argjson chromium_flow_ids "$(jq -ec '.flow_ids' "$CHROMIUM_JSON")" \
  --argjson firefox_flow_ids "$(jq -ec '.flow_ids' "$FIREFOX_JSON")" \
  --argjson flow_ids "$(jq -cn --argjson chromium "$(jq -ec '.flow_ids' "$CHROMIUM_JSON")" --argjson firefox "$(jq -ec '.flow_ids' "$FIREFOX_JSON")" '$chromium + $firefox | unique')" \
  --argjson count "$(( $(jq -er '.ready_flow_count' "$CHROMIUM_JSON") + $(jq -er '.ready_flow_count' "$FIREFOX_JSON") ))" \
  --argjson real_host "$REAL_BROWSER_HOST" \
  '{schema_version:1,real_host:$real_host,base_url:$base_url,chromium:$chromium,firefox:$firefox,chromium_flow_ids:$chromium_flow_ids,firefox_flow_ids:$firefox_flow_ids,flow_ids:$flow_ids,ready_flow_count:$count}' \
  >"$tmp_browser"
mv -- "$tmp_browser" "$EVIDENCE_DIR/browser.json"

HEALTH_FINAL_JSON="$(curl --fail --silent --show-error "$BASE_URL/health")"
DB_FINGERPRINT_FINAL="$(jq -er '.data.db_fingerprint' <<<"$HEALTH_FINAL_JSON")"
tmp_host_final="$EVIDENCE_DIR/.host-final.json.$HOST_PID.tmp"
jq --arg fingerprint "$DB_FINGERPRINT_FINAL" --arg health "$(jq -c '.data' <<<"$HEALTH_FINAL_JSON")" \
  '.db_fingerprint_before=$fingerprint | .health=($health|fromjson) | .db_fingerprint_after_browser=$fingerprint' \
  "$EVIDENCE_DIR/host.json" >"$tmp_host_final"
mv -- "$tmp_host_final" "$EVIDENCE_DIR/host.json"

if [[ "${KANBAN_RELEASE_SKIP_RECEIPT:-0}" != "1" ]]; then
  "$LOCK" -- cargo run --locked -p xtask --bin xtask -- release receipt \
    --root "$ROOT" --evidence "$EVIDENCE_DIR" --artifact "$ROOT/apps/web/dist" \
    --out "$EVIDENCE_DIR/release-proof-receipt.json"
fi

echo "ok: 09A real-host browser proof passed; evidence=$EVIDENCE_DIR host_pid=$HOST_PID"
