#!/usr/bin/env bash
set -euo pipefail
umask 077

ROOT="$(cd "$(dirname "$0")/.." && pwd -P)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
PORT="${KANBAN_RELEASE_PORT:-18721}"
BASE_URL="http://127.0.0.1:$PORT"
START_SHA="$(git -C "$ROOT" rev-parse HEAD)"
START_CLEAN=false
if [[ -z "$(git -C "$ROOT" status --porcelain=v1)" ]]; then START_CLEAN=true; fi
RUN_ID="09a-$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n')"

TMP_ROOT="$(mktemp -d "/tmp/kanban-release-09a.XXXXXX")"
chmod 700 -- "$TMP_ROOT"
TMP_ROOT="$(realpath -e -- "$TMP_ROOT")"
DB_PATH="$TMP_ROOT/canonical.db"
HOST_LOG="$TMP_ROOT/host.log"
HOST_PID=""
HOST_START=""
HOST_CMDLINE=""
HOST_ARGV_JSON=""
HOST_BINARY_SHA=""
EVIDENCE_DIR_INPUT="${KANBAN_RELEASE_EVIDENCE_DIR:-$ROOT/output/release}"
error() { echo "error: $*" >&2; exit 1; }
if [[ "$EVIDENCE_DIR_INPUT" = /* ]]; then
  EVIDENCE_DIR_RAW="$EVIDENCE_DIR_INPUT"
else
  EVIDENCE_DIR_RAW="$ROOT/$EVIDENCE_DIR_INPUT"
fi
case "$EVIDENCE_DIR_RAW" in
  *"/../"*|*"/.."|../*|..|*"/./"*|*"/.."|./*) error "evidence dir must not contain traversal or dot components" ;;
esac
reject_symlink_chain() {
  local path="$1" current="/" component
  [[ "$path" = /* ]] || error "path must be absolute: $path"
  IFS='/' read -r -a components <<< "${path#/}"
  for component in "${components[@]}"; do
    [[ -z "$component" ]] && continue
    current="$current$component"
    [[ ! -L "$current" ]] || error "path chain contains symlink: $current"
    current="$current/"
  done
}
DEFAULT_EVIDENCE_DIR="$ROOT/output/release"

validate_evidence_dir() {
  local parent owner mode
  [[ "$EVIDENCE_DIR" != "/" && "$EVIDENCE_DIR" != "/tmp" && "$EVIDENCE_DIR" != "$ROOT" ]] || error "evidence dir too broad: $EVIDENCE_DIR"
  reject_symlink_chain "$EVIDENCE_DIR"
  if [[ "$EVIDENCE_DIR" == "$DEFAULT_EVIDENCE_DIR" ]]; then
    parent="$(dirname "$EVIDENCE_DIR")"
    reject_symlink_chain "$parent"
    mkdir -p -m 700 -- "$EVIDENCE_DIR"
  else
    [[ "$EVIDENCE_DIR" == /tmp/* ]] || error "evidence dir must be $DEFAULT_EVIDENCE_DIR or a launcher-owned /tmp directory"
    [[ -d "$EVIDENCE_DIR" ]] || error "custom evidence dir must already exist: $EVIDENCE_DIR"
    owner="$(stat -c '%u' -- "$EVIDENCE_DIR")"
    mode="$(stat -c '%a' -- "$EVIDENCE_DIR")"
    [[ "$owner" == "$(id -u)" && "$mode" == "700" ]] || error "custom evidence dir must be uid-owned mode 700: $EVIDENCE_DIR"
  fi
  owner="$(stat -c '%u' -- "$EVIDENCE_DIR")"
  mode="$(stat -c '%a' -- "$EVIDENCE_DIR")"
  [[ "$owner" == "$(id -u)" && "$mode" == "700" ]] || error "evidence dir must be uid-owned mode 700: $EVIDENCE_DIR"
  [[ -d "$EVIDENCE_DIR" && ! -L "$EVIDENCE_DIR" ]] || error "evidence dir is not a real directory: $EVIDENCE_DIR"
}

reject_symlink_chain "$EVIDENCE_DIR_RAW"
EVIDENCE_DIR="$(realpath -m -- "$EVIDENCE_DIR_RAW")"
[[ "$EVIDENCE_DIR" == "$EVIDENCE_DIR_RAW" ]] || error "evidence dir must already be canonical: $EVIDENCE_DIR_RAW"

validate_evidence_destination() {
  local path="$1" metadata links
  reject_symlink_chain "$EVIDENCE_DIR"
  if [[ -e "$path" || -L "$path" ]]; then
    [[ ! -L "$path" ]] || error "evidence destination is symlink: $path"
    [[ -f "$path" ]] || error "evidence destination is not regular: $path"
    links="$(stat -c '%h' -- "$path")"
    [[ "$links" == "1" ]] || error "evidence destination is hardlinked: $path"
  fi
}

atomic_write_file() {
  local destination="$1" contents="$2" temporary
  validate_evidence_destination "$destination"
  temporary="$(mktemp "$EVIDENCE_DIR/.release-proof.XXXXXX")"
  chmod 600 -- "$temporary"
  printf '%s\n' "$contents" >"$temporary"
  sync -d "$temporary" 2>/dev/null || true
  mv -- "$temporary" "$destination"
}

stat_identity() { stat -c '%d:%i' -- "$1"; }
validate_db_file() {
  local path="$1"
  [[ -f "$path" && ! -L "$path" ]] || error "canonical DB must be a regular non-symlink file: $path"
  [[ "$(stat -c '%u' -- "$path")" == "$(id -u)" && "$(stat -c '%h' -- "$path")" == "1" ]] || error "canonical DB must be uid-owned with nlink=1: $path"
}
process_start() { awk '{print $22}' "/proc/$1/stat" 2>/dev/null; }
process_state() { awk '{print $3}' "/proc/$1/stat" 2>/dev/null; }
process_identity_matches() {
  local pid="$1" expected="$2" current
  current="$(process_start "$pid" || true)"
  [[ -n "$current" && "$current" == "$expected" ]]
}
process_alive_with_identity() {
  local pid="$1" expected="$2" state
  kill -0 "$pid" 2>/dev/null || return 1
  state="$(process_state "$pid" || true)"
  [[ "$state" != "Z" ]] || return 1
  process_identity_matches "$pid" "$expected"
}
signal_group_if_identity() {
  local signal="$1" pid="$2" expected="$3"
  process_alive_with_identity "$pid" "$expected" || return 1
  kill -"$signal" -- "-$pid" 2>/dev/null
}
wait_for_exit() {
  local pid="$1" expected="$2" attempts="$3" current state
  for _ in $(seq 1 "$attempts"); do
    if ! kill -0 "$pid" 2>/dev/null; then return 0; fi
    state="$(process_state "$pid" || true)"
    [[ "$state" == "Z" ]] && return 0
    current="$(process_start "$pid" || true)"
    [[ "$current" == "$expected" ]] || return 0
    sleep 0.1
  done
  return 1
}
reap_if_exited() {
  local pid="$1" expected="$2"
  if ! process_alive_with_identity "$pid" "$expected"; then
    wait "$pid" 2>/dev/null || true
  fi
}
stop_host() {
  local pid="$HOST_PID" expected="$HOST_START"
  [[ -n "$pid" ]] || return 0
  if kill -0 "$pid" 2>/dev/null && [[ "$(process_state "$pid" || true)" != "Z" ]] && ! process_identity_matches "$pid" "$expected"; then
    echo "error: refusing to signal PID with starttime mismatch: $pid" >&2
    return 1
  fi
  if process_alive_with_identity "$pid" "$expected"; then
    signal_group_if_identity INT "$pid" "$expected" || true
    if ! wait_for_exit "$pid" "$expected" 80; then
      signal_group_if_identity TERM "$pid" "$expected" || true
      if ! wait_for_exit "$pid" "$expected" 40; then
        signal_group_if_identity KILL "$pid" "$expected" || true
        if ! wait_for_exit "$pid" "$expected" 20; then
          echo "error: kanban serve did not exit after bounded INT/TERM/KILL" >&2
          return 1
        fi
      fi
    fi
  fi
  reap_if_exited "$pid" "$expected"
  HOST_PID=""
  HOST_START=""
  HOST_CMDLINE=""
  return 0
}
cleanup() {
  local exit_status="$?" stop_status=0
  set +e
  stop_host || stop_status=$?
  if [[ -f "$HOST_LOG" && -d "$EVIDENCE_DIR" && ! -L "$EVIDENCE_DIR" ]]; then
    atomic_write_file "$EVIDENCE_DIR/host.log" "$(<"$HOST_LOG")" || true
  fi
  if [[ "$stop_status" -eq 0 && "$TMP_ROOT" == /tmp/kanban-release-09a.* && -d "$TMP_ROOT" && ! -L "$TMP_ROOT" ]]; then
    rm -rf -- "$TMP_ROOT"
  fi
  if [[ "$stop_status" -ne 0 && "$exit_status" -eq 0 ]]; then exit_status="$stop_status"; fi
  trap - EXIT
  exit "$exit_status"
}
trap cleanup EXIT

for tool in curl jq pnpm setsid od stat sha256sum realpath sync; do
  command -v "$tool" >/dev/null 2>&1 || error "release 09A requires $tool"
done
[[ "$(stat -c '%a' -- "$TMP_ROOT")" == "700" && ! -L "$TMP_ROOT" ]] || error "launcher temp root must be private mode 700"
validate_evidence_dir

if curl --silent --max-time 0.5 "$BASE_URL/health" >/dev/null 2>&1; then
  error "release port already serves a host: $BASE_URL"
fi
for evidence_name in host.json browser.json browser-chromium.json browser-firefox.json host.log release-proof-receipt.json; do
  evidence_path="$EVIDENCE_DIR/$evidence_name"
  validate_evidence_destination "$evidence_path"
  if [[ -e "$evidence_path" ]]; then rm -f -- "$evidence_path"; fi
done

just --justfile "$ROOT/justfile" web-build
"$LOCK" -- cargo build --locked -p kanban-cli
TARGET_DIR="$("$LOCK" --print-target-dir)"
KANBAN="$TARGET_DIR/debug/kanban"
[[ -x "$KANBAN" && ! -L "$KANBAN" ]] || error "kanban binary missing: $KANBAN"
KANBAN_BINARY_SHA256="sha256:$(sha256sum -- "$KANBAN" | awk '{print $1}')"

capture_host() {
  local pid="$1"
  for _ in $(seq 1 50); do
    if [[ -r "/proc/$pid/stat" && -r "/proc/$pid/cmdline" ]]; then break; fi
    sleep 0.1
  done
  HOST_START="$(process_start "$pid")"
  HOST_CMDLINE="$(tr '\0' ' ' <"/proc/$pid/cmdline")"
  HOST_CMDLINE="$(sed 's/[[:space:]]*$//' <<<"$HOST_CMDLINE")"
  HOST_ARGV_JSON="$(tr '\0' '\n' <"/proc/$pid/cmdline" | jq -R -s 'split("\n") | map(select(length > 0))')"
  HOST_BINARY_SHA="sha256:$(sha256sum -- "/proc/$pid/exe" | awk '{print $1}')"
  [[ -n "$HOST_START" && "$HOST_CMDLINE" == *kanban* && "$HOST_CMDLINE" == *serve* ]] || error "host PID is not kanban serve"
  [[ "$HOST_BINARY_SHA" == "$KANBAN_BINARY_SHA256" ]] || error "live host executable differs from built kanban binary"
}
start_host() {
  setsid env KANBAN_ACTOR=release-09a "$KANBAN" --db "$DB_PATH" serve --host 127.0.0.1 --port "$PORT" --web-dir "$ROOT/apps/web/dist" >>"$HOST_LOG" 2>&1 &
  HOST_PID="$!"
  capture_host "$HOST_PID"
}
start_host
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

HEALTH_BEFORE_JSON="$(curl --fail --silent --show-error "$BASE_URL/health")"
RUNTIME_JSON="$(curl --fail --silent --show-error "$BASE_URL/app/runtime.json")"
MANIFEST_JSON="$(curl --fail --silent --show-error "$BASE_URL/app/manifest.json")"
DOCTOR_JSON="$(curl --fail --silent --show-error "$BASE_URL/api/v1/maintenance/doctor")"
MIGRATION_BEFORE="$(jq -er '.data.migration_version // .data.user_version' <<<"$DOCTOR_JSON")"
DB_PATH_BEFORE_RESTART="$(jq -er '.data.db_path' <<<"$HEALTH_BEFORE_JSON")"
DB_FINGERPRINT_BEFORE_RESTART="$(jq -er '.data.db_fingerprint' <<<"$HEALTH_BEFORE_JSON")"
[[ "$DB_PATH_BEFORE_RESTART" == "$DB_PATH" ]] || error "health db_path before restart differs from launcher DB"
validate_db_file "$DB_PATH_BEFORE_RESTART"
DB_IDENTITY_BEFORE_RESTART="$(stat_identity "$DB_PATH_BEFORE_RESTART")"

stop_host
if curl --silent --max-time 0.5 "$BASE_URL/health" >/dev/null 2>&1; then error "canonical host port remained open after stop"; fi
start_host
[[ "$HOST_PID" != "$HOST_PID_BEFORE" || "$HOST_START" != "$HOST_START_BEFORE" ]] || error "restart did not produce a distinct PID identity"
curl --fail --silent --show-error --retry 120 --retry-all-errors --retry-delay 1 "$BASE_URL/health" >/dev/null
HEALTH_AFTER_RESTART_JSON="$(curl --fail --silent --show-error "$BASE_URL/health")"
RUNTIME_JSON="$(curl --fail --silent --show-error "$BASE_URL/app/runtime.json")"
MANIFEST_JSON="$(curl --fail --silent --show-error "$BASE_URL/app/manifest.json")"
DB_PATH_AFTER_RESTART="$(jq -er '.data.db_path' <<<"$HEALTH_AFTER_RESTART_JSON")"
DB_FINGERPRINT_AFTER_RESTART="$(jq -er '.data.db_fingerprint' <<<"$HEALTH_AFTER_RESTART_JSON")"
[[ "$DB_PATH_AFTER_RESTART" == "$DB_PATH" ]] || error "health db_path after restart differs from launcher DB"
validate_db_file "$DB_PATH_AFTER_RESTART"
DB_IDENTITY_AFTER_RESTART="$(stat_identity "$DB_PATH_AFTER_RESTART")"
[[ "$DB_IDENTITY_BEFORE_RESTART" == "$DB_IDENTITY_AFTER_RESTART" ]] || error "DB dev:inode changed across restart"
SEED_JSON_AFTER="$(KANBAN_SERVER_URL="$BASE_URL" "$KANBAN" --json --board default task show t_release_seed)"
SEED_TITLE_AFTER="$(jq -er '.data.title' <<<"$SEED_JSON_AFTER")"
[[ "$SEED_TITLE_AFTER" == "Seed release task" ]] || error "seed task did not survive host restart"
SEED_RECOVERED=true

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
NOSNIFF_CHECKED=false
REFERRER_POLICY_CHECKED=false
FRAME_DENY_CHECKED=false
if [[ "$HOST_NEGATIVE_STATUS" == "400" || "$HOST_NEGATIVE_STATUS" == "403" ]]; then HOST_NEGATIVE_CHECKED=true; fi
if [[ "$ORIGIN_NEGATIVE_STATUS" == "400" || "$ORIGIN_NEGATIVE_STATUS" == "403" ]] && [[ -z "$ORIGIN_NEGATIVE_ACAO" ]]; then ORIGIN_NEGATIVE_CHECKED=true; fi
if [[ "$ORIGIN_NULL_STATUS" == "400" || "$ORIGIN_NULL_STATUS" == "403" ]] && [[ -z "$ORIGIN_NULL_ACAO" ]]; then ORIGIN_NULL_REJECTED=true; fi
if grep -Fq "default-src 'self'" <<<"$CSP_HEADER" && grep -Fq "script-src 'self'" <<<"$CSP_HEADER"; then CSP_CHECKED=true; fi
SECURITY_HEADERS="$(curl --silent --show-error --dump-header - -o /dev/null "$BASE_URL/app/")"
if grep -Eiq '^x-content-type-options:[[:space:]]*nosniff[[:space:]]*$' <<<"$SECURITY_HEADERS"; then NOSNIFF_CHECKED=true; fi
if grep -Eiq '^referrer-policy:[[:space:]]*no-referrer[[:space:]]*$' <<<"$SECURITY_HEADERS"; then REFERRER_POLICY_CHECKED=true; fi
if grep -Eiq '^x-frame-options:[[:space:]]*DENY[[:space:]]*$' <<<"$SECURITY_HEADERS"; then FRAME_DENY_CHECKED=true; fi
[[ "$HOST_NEGATIVE_CHECKED" == true && "$ORIGIN_NEGATIVE_CHECKED" == true && "$ORIGIN_NULL_REJECTED" == true ]] || error "host/origin negative probe failed"
[[ "$CSP_CHECKED" == true && "$NOSNIFF_CHECKED" == true && "$REFERRER_POLICY_CHECKED" == true && "$FRAME_DENY_CHECKED" == true ]] || error "app security headers probe failed"
REAL_HOST_CHECKED=true

tmp_host="$(mktemp "$EVIDENCE_DIR/.host.json.XXXXXX")"
chmod 600 -- "$tmp_host"
jq -n \
  --arg run_id "$RUN_ID" \
  --arg start_sha "$START_SHA" \
  --argjson start_clean "$START_CLEAN" \
  --arg base_url "$BASE_URL" \
  --argjson pid "$HOST_PID" \
  --argjson pid_before "$HOST_PID_BEFORE" \
  --arg start_before "$HOST_START_BEFORE" \
  --arg start_after "$HOST_START" \
  --arg command_line_before "$HOST_CMDLINE_BEFORE" \
  --arg command_line "$HOST_CMDLINE" \
  --argjson argv "$HOST_ARGV_JSON" \
  --arg binary_path "$KANBAN" \
  --arg binary_sha256 "$KANBAN_BINARY_SHA256" \
  --arg live_binary_sha256 "$HOST_BINARY_SHA" \
  --arg health_before_restart "$(jq -c '.data' <<<"$HEALTH_BEFORE_JSON")" \
  --arg health_after_restart "$(jq -c '.data' <<<"$HEALTH_AFTER_RESTART_JSON")" \
  --arg runtime "$RUNTIME_JSON" \
  --arg manifest "$MANIFEST_JSON" \
  --arg db_path_before_restart "$DB_PATH_BEFORE_RESTART" \
  --arg db_path_after_restart "$DB_PATH_AFTER_RESTART" \
  --arg db_path_after_browser "$DB_PATH_AFTER_RESTART" \
  --arg db_identity_before_restart "$DB_IDENTITY_BEFORE_RESTART" \
  --arg db_identity_after_restart "$DB_IDENTITY_AFTER_RESTART" \
  --arg db_identity_after_browser "$DB_IDENTITY_AFTER_RESTART" \
  --arg db_fingerprint_before_restart "$DB_FINGERPRINT_BEFORE_RESTART" \
  --arg db_fingerprint_after_restart "$DB_FINGERPRINT_AFTER_RESTART" \
  --arg seed_title "$SEED_TITLE_AFTER" \
  --argjson migration_before "$MIGRATION_BEFORE" \
  --argjson host_negative_checked "$HOST_NEGATIVE_CHECKED" \
  --argjson origin_negative_checked "$ORIGIN_NEGATIVE_CHECKED" \
  --argjson origin_null_rejected "$ORIGIN_NULL_REJECTED" \
  --argjson csp_checked "$CSP_CHECKED" \
  --argjson nosniff_checked "$NOSNIFF_CHECKED" \
  --argjson referrer_policy_checked "$REFERRER_POLICY_CHECKED" \
  --argjson frame_deny_checked "$FRAME_DENY_CHECKED" \
  --argjson real_host "$REAL_HOST_CHECKED" \
  --argjson seed_recovered "$SEED_RECOVERED" \
  '{schema_version:2,run_id:$run_id,start_sha:$start_sha,start_clean:$start_clean,real_host:$real_host,base_url:$base_url,pid:$pid,pid_before:$pid_before,start_before:$start_before,start_after:$start_after,command_line:$command_line,command_line_before:$command_line_before,argv:$argv,binary_path:$binary_path,binary_sha256:$binary_sha256,live_binary_sha256:$live_binary_sha256,health_before_restart:($health_before_restart|fromjson),health_after_restart:($health_after_restart|fromjson),runtime:($runtime|fromjson),manifest:($manifest|fromjson),db_path_before_restart:$db_path_before_restart,db_path_after_restart:$db_path_after_restart,db_path_after_browser:$db_path_after_browser,db_identity_before_restart:$db_identity_before_restart,db_identity_after_restart:$db_identity_after_restart,db_identity_after_browser:$db_identity_after_browser,db_fingerprint_before_restart:$db_fingerprint_before_restart,db_fingerprint_after_restart:$db_fingerprint_after_restart,seed_title:$seed_title,seed_recovered:$seed_recovered,migration_before:$migration_before,host_negative_checked:$host_negative_checked,origin_negative_checked:$origin_negative_checked,origin_null_rejected:$origin_null_rejected,csp_checked:$csp_checked,nosniff_checked:$nosniff_checked,referrer_policy_checked:$referrer_policy_checked,frame_deny_checked:$frame_deny_checked}' \
  >"$tmp_host"
mv -- "$tmp_host" "$EVIDENCE_DIR/host.json"

KANBAN_RELEASE_PROOF=1 KANBAN_RELEASE_RUN_ID="$RUN_ID" KANBAN_RELEASE_BASE_URL="$BASE_URL" KANBAN_RELEASE_EVIDENCE_DIR="$EVIDENCE_DIR" \
  pnpm --filter '@kanban-tool/web' exec playwright test --project=chromium --project=firefox

CHROMIUM_JSON="$EVIDENCE_DIR/browser-chromium.json"
FIREFOX_JSON="$EVIDENCE_DIR/browser-firefox.json"
[[ -f "$CHROMIUM_JSON" && -f "$FIREFOX_JSON" ]] || error "browser evidence missing"
[[ "$(jq -er '.run_id' "$CHROMIUM_JSON")" == "$RUN_ID" && "$(jq -er '.run_id' "$FIREFOX_JSON")" == "$RUN_ID" ]] || error "browser evidence run_id mismatch"
CHROMIUM_REAL_HOST="$(jq -er '.real_host' "$CHROMIUM_JSON")"
FIREFOX_REAL_HOST="$(jq -er '.real_host' "$FIREFOX_JSON")"
[[ "$CHROMIUM_REAL_HOST" == true && "$FIREFOX_REAL_HOST" == true ]] || error "browser lanes did not prove real host"
tmp_browser="$(mktemp "$EVIDENCE_DIR/.browser.json.XXXXXX")"
chmod 600 -- "$tmp_browser"
jq -n \
  --arg run_id "$RUN_ID" \
  --arg base_url "$BASE_URL" \
  --arg chromium "$(jq -er '.chromium' "$CHROMIUM_JSON")" \
  --arg firefox "$(jq -er '.firefox' "$FIREFOX_JSON")" \
  --argjson chromium_flow_ids "$(jq -ec '.flow_ids' "$CHROMIUM_JSON")" \
  --argjson firefox_flow_ids "$(jq -ec '.flow_ids' "$FIREFOX_JSON")" \
  --argjson flow_ids "$(jq -cn --argjson chromium "$(jq -ec '.flow_ids' "$CHROMIUM_JSON")" --argjson firefox "$(jq -ec '.flow_ids' "$FIREFOX_JSON")" '$chromium + $firefox | unique')" \
  --argjson created_task_ids "$(jq -cn --argjson chromium "$(jq -ec '.created_task_ids' "$CHROMIUM_JSON")" --argjson firefox "$(jq -ec '.created_task_ids' "$FIREFOX_JSON")" '$chromium + $firefox')" \
  --argjson created_task_titles "$(jq -cn --argjson chromium "$(jq -ec '.created_task_titles' "$CHROMIUM_JSON")" --argjson firefox "$(jq -ec '.created_task_titles' "$FIREFOX_JSON")" '$chromium + $firefox')" \
  --argjson created_task_projects "$(jq -cn --argjson chromium "$(jq -ec '.created_task_projects' "$CHROMIUM_JSON")" --argjson firefox "$(jq -ec '.created_task_projects' "$FIREFOX_JSON")" '$chromium + $firefox')" \
  --argjson created_task_board_slugs "$(jq -cn --argjson chromium "$(jq -ec '.created_task_board_slugs' "$CHROMIUM_JSON")" --argjson firefox "$(jq -ec '.created_task_board_slugs' "$FIREFOX_JSON")" '$chromium + $firefox')" \
  --argjson count "$(( $(jq -er '.ready_flow_count' "$CHROMIUM_JSON") + $(jq -er '.ready_flow_count' "$FIREFOX_JSON") ))" \
  --argjson real_host true \
  '{schema_version:2,run_id:$run_id,real_host:$real_host,base_url:$base_url,chromium:$chromium,firefox:$firefox,chromium_flow_ids:$chromium_flow_ids,firefox_flow_ids:$firefox_flow_ids,flow_ids:$flow_ids,ready_flow_count:$count,created_task_ids:$created_task_ids,created_task_titles:$created_task_titles,created_task_projects:$created_task_projects,created_task_board_slugs:$created_task_board_slugs}' \
  >"$tmp_browser"
mv -- "$tmp_browser" "$EVIDENCE_DIR/browser.json"

HEALTH_AFTER_BROWSER_JSON="$(curl --fail --silent --show-error "$BASE_URL/health")"
DB_PATH_AFTER_BROWSER="$(jq -er '.data.db_path' <<<"$HEALTH_AFTER_BROWSER_JSON")"
DB_FINGERPRINT_AFTER_BROWSER="$(jq -er '.data.db_fingerprint' <<<"$HEALTH_AFTER_BROWSER_JSON")"
[[ "$DB_PATH_AFTER_BROWSER" == "$DB_PATH" ]] || error "health db_path after browser differs from launcher DB"
validate_db_file "$DB_PATH_AFTER_BROWSER"
DB_IDENTITY_AFTER_BROWSER="$(stat_identity "$DB_PATH_AFTER_BROWSER")"
[[ "$DB_IDENTITY_BEFORE_RESTART" == "$DB_IDENTITY_AFTER_BROWSER" && "$DB_IDENTITY_AFTER_RESTART" == "$DB_IDENTITY_AFTER_BROWSER" ]] || error "DB dev:inode changed after browser lane"
tmp_host_final="$(mktemp "$EVIDENCE_DIR/.host-final.json.XXXXXX")"
chmod 600 -- "$tmp_host_final"
jq \
  --arg health_after_browser "$(jq -c '.data' <<<"$HEALTH_AFTER_BROWSER_JSON")" \
  --arg db_path_after_browser "$DB_PATH_AFTER_BROWSER" \
  --arg db_identity_after_browser "$DB_IDENTITY_AFTER_BROWSER" \
  --arg db_fingerprint_after_browser "$DB_FINGERPRINT_AFTER_BROWSER" \
  '.health_after_browser=($health_after_browser|fromjson) | .db_path_after_browser=$db_path_after_browser | .db_identity_after_browser=$db_identity_after_browser | .db_fingerprint_after_browser=$db_fingerprint_after_browser' \
  "$EVIDENCE_DIR/host.json" >"$tmp_host_final"
mv -- "$tmp_host_final" "$EVIDENCE_DIR/host.json"

if [[ "${KANBAN_RELEASE_SKIP_RECEIPT:-0}" != "1" ]]; then
  "$LOCK" -- cargo run --locked -p xtask --bin xtask -- release receipt \
    --root "$ROOT" --evidence "$EVIDENCE_DIR" --artifact "$ROOT/apps/web/dist" \
    --out "$EVIDENCE_DIR/release-proof-receipt.json"
fi

echo "ok: 09A real-host browser proof passed; run_id=$RUN_ID evidence=$EVIDENCE_DIR host_pid=$HOST_PID"
