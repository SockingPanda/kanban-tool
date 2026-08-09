#!/usr/bin/env bash
set -euo pipefail
umask 077

ROOT="$(cd "$(dirname "$0")/.." && pwd -P)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
PORT="${KANBAN_RELEASE_09D_PORT:-18729}"
BASE_URL="http://127.0.0.1:$PORT"
START_SHA="$(git -C "$ROOT" rev-parse HEAD)"
START_CLEAN=false
if [[ -z "$(git -C "$ROOT" status --porcelain=v1)" ]]; then START_CLEAN=true; fi
RUN_ID="09d-$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n')"
FORMAL_MODE="${KANBAN_RELEASE_09D_FORMAL:-true}"
error() { echo "error: $*" >&2; exit 1; }
[[ "$FORMAL_MODE" == "true" || "$FORMAL_MODE" == "false" ]] || error "KANBAN_RELEASE_09D_FORMAL must be true or false"

TMP_ROOT="$(mktemp -d "/tmp/kanban-release-09d.XXXXXX")"
chmod 700 -- "$TMP_ROOT"
TMP_ROOT="$(realpath -e -- "$TMP_ROOT")"
TMP_ROOT_RECORDED="$TMP_ROOT"
DB_PATH="$TMP_ROOT/canonical.db"
HOST_LOG="$TMP_ROOT/host.log"
HOST_PID=""
HOST_STARTTIME=""
HOST_CMDLINE=""
HOST_ARGV_JSON=""
HOST_PID_RECORDED=""
HOST_STARTTIME_RECORDED=""
DB_IDENTITY_BEFORE=""
EVIDENCE_DIR="$ROOT/output/release/09d"

process_start() { awk '{print $22}' "/proc/$1/stat" 2>/dev/null; }
process_state() { awk '{print $3}' "/proc/$1/stat" 2>/dev/null; }
process_alive() {
  local pid="$1" expected="$2"
  kill -0 "$pid" 2>/dev/null || return 1
  [[ "$(process_state "$pid" || true)" != "Z" ]] || return 1
  [[ "$(process_start "$pid" || true)" == "$expected" ]]
}
wait_for_exit() {
  local pid="$1" expected="$2" attempts="$3"
  for _ in $(seq 1 "$attempts"); do
    if ! kill -0 "$pid" 2>/dev/null; then return 0; fi
    [[ "$(process_state "$pid" || true)" == "Z" ]] && return 0
    [[ "$(process_start "$pid" || true)" == "$expected" ]] || return 0
    sleep 0.1
  done
  return 1
}
stop_host() {
  local pid="$HOST_PID" expected="$HOST_STARTTIME"
  [[ -n "$pid" ]] || return 0
  if process_alive "$pid" "$expected"; then
    kill -INT -- "-$pid" 2>/dev/null || true
    if ! wait_for_exit "$pid" "$expected" 80; then
      kill -TERM -- "-$pid" 2>/dev/null || true
      if ! wait_for_exit "$pid" "$expected" 40; then
        kill -KILL -- "-$pid" 2>/dev/null || true
        wait_for_exit "$pid" "$expected" 20 || return 1
      fi
    fi
  fi
  wait "$pid" 2>/dev/null || true
  HOST_PID=""
  HOST_STARTTIME=""
}
cleanup() {
  local exit_status="$?" stop_status=0
  trap - EXIT
  set +e
  stop_host || stop_status=$?
  if [[ "$stop_status" -eq 0 && -n "$TMP_ROOT" && -d "$TMP_ROOT" && "$TMP_ROOT" == /tmp/kanban-release-09d.* && ! -L "$TMP_ROOT" ]]; then
    rm -rf -- "$TMP_ROOT"
  fi
  if [[ "$stop_status" -ne 0 && "$exit_status" -eq 0 ]]; then exit_status="$stop_status"; fi
  exit "$exit_status"
}
trap cleanup EXIT

for tool in cargo curl jq node od realpath sha256sum stat setsid pnpm ss; do
  command -v "$tool" >/dev/null 2>&1 || error "release 09D requires $tool"
done
[[ "$PORT" != "8721" && "$PORT" != "18721" ]] || error "09D port must be independent from 8721/18721"
port_listeners() {
  ss -H -ltn | awk -v port=":$PORT" '$4 ~ port "$" || $4 ~ /\]:/ port "$" { print }'
}
[[ -z "$(port_listeners)" ]] || error "release port already has a listener: $BASE_URL"

for private_dir in "$ROOT/output" "$ROOT/output/release" "$EVIDENCE_DIR"; do
  [[ ! -L "$private_dir" ]] || error "09D evidence path component is a symlink: $private_dir"
  mkdir -p -- "$private_dir"
  [[ -d "$private_dir" && ! -L "$private_dir" ]] || error "09D evidence path component is not a real directory: $private_dir"
  [[ "$(stat -c '%u' -- "$private_dir")" == "$(id -u)" ]] || error "09D evidence path component owner mismatch: $private_dir"
done
chmod 700 -- "$EVIDENCE_DIR"
[[ "$(stat -c '%a' -- "$EVIDENCE_DIR")" == "700" ]] || error "09D evidence leaf must be mode 700: $EVIDENCE_DIR"

remove_stale_regular() {
  local candidate="$1"
  [[ -e "$candidate" || -L "$candidate" ]] || return 0
  [[ ! -L "$candidate" ]] || error "09D stale evidence path is a symlink: $candidate"
  [[ -f "$candidate" ]] || error "09D stale evidence path is not a regular file: $candidate"
  [[ "$(stat -c '%h' -- "$candidate")" == "1" ]] || error "09D stale evidence path has unexpected link count: $candidate"
  [[ "$(stat -c '%u' -- "$candidate")" == "$(id -u)" ]] || error "09D stale evidence path owner mismatch: $candidate"
  rm -f -- "$candidate"
}

while IFS= read -r -d '' stale_tmp; do
  error "09D stale evidence temporary file exists; refusing to overwrite: $stale_tmp"
done < <(find "$EVIDENCE_DIR" -maxdepth 1 \( -name '.release-09d-evidence.tmp' -o -name 'release-09d-*.tmp' \) -print0)

for stale_json in \
  "$EVIDENCE_DIR/release-09d-small-perf-sse-chromium.json" \
  "$EVIDENCE_DIR/release-09d-small-sse-firefox.json" \
  "$EVIDENCE_DIR/release-09d-functional-2k-chromium.json" \
  "$EVIDENCE_DIR/release-09d-functional-2k-firefox.json" \
  "$EVIDENCE_DIR/release-09d-functional-2k-chromium-chromium.json" \
  "$EVIDENCE_DIR/release-09d-functional-2k-firefox-firefox.json" \
  "$EVIDENCE_DIR/release-09d-stress-5k-chromium.json" \
  "$EVIDENCE_DIR/release-09d-stress-5k-firefox.json" \
  "$EVIDENCE_DIR/release-09d-stress-5k-chromium-chromium.json" \
  "$EVIDENCE_DIR/release-09d-stress-5k-firefox-firefox.json" \
  "$EVIDENCE_DIR/release-09d-evidence.json"; do
  remove_stale_regular "$stale_json"
done

while IFS= read -r -d '' stale_log; do
  [[ ! -L "$stale_log" ]] || error "09D stale host log is a symlink: $stale_log"
  [[ -f "$stale_log" ]] || error "09D stale host log is not a regular file: $stale_log"
  [[ "$(stat -c '%h' -- "$stale_log")" == "1" ]] || error "09D stale host log has unexpected link count: $stale_log"
  [[ "$(stat -c '%a' -- "$stale_log")" == "600" ]] || error "09D stale host log must be mode 600: $stale_log"
  [[ "$(stat -c '%u' -- "$stale_log")" == "$(id -u)" ]] || error "09D stale host log owner mismatch: $stale_log"
  rm -f -- "$stale_log"
done < <(find "$EVIDENCE_DIR" -maxdepth 1 -name 'host-09d-*.log' -print0)

just --justfile "$ROOT/justfile" web-build
"$LOCK" -- cargo build --locked -p kanban-cli
TARGET_DIR="$("$LOCK" --print-target-dir)"
KANBAN="$TARGET_DIR/debug/kanban"
[[ -x "$KANBAN" && ! -L "$KANBAN" ]] || error "kanban binary missing: $KANBAN"
KANBAN_BINARY_SHA256="sha256:$(sha256sum -- "$KANBAN" | awk '{print $1}')"
ARTIFACT_MANIFEST="$ROOT/apps/web/dist/manifest.json"
[[ -f "$ARTIFACT_MANIFEST" && ! -L "$ARTIFACT_MANIFEST" ]] || error "Web artifact manifest missing"
ARTIFACT_BUILD_ID="$(jq -er '.buildId' -- "$ARTIFACT_MANIFEST")"
ARTIFACT_MANIFEST_SHA256="sha256:$(sha256sum -- "$ARTIFACT_MANIFEST" | awk '{print $1}')"
node "$ROOT/scripts/release-proof-09d.mjs" self-test >/dev/null
INITIAL_JSON="$(node "$ROOT/scripts/release-proof-09d.mjs" artifact --root "$ROOT")"
INITIAL_BUILD_ID="$(jq -er '.build_id' <<<"$INITIAL_JSON")"
[[ "$INITIAL_BUILD_ID" == "$ARTIFACT_BUILD_ID" ]] || error "artifact helper build_id differs from manifest buildId"
INITIAL_BROTLI_BYTES="$(jq -er '.brotli_bytes' <<<"$INITIAL_JSON")"
INITIAL_FILES_JSON="$(jq -c '.files' <<<"$INITIAL_JSON")"
[[ "$INITIAL_BROTLI_BYTES" -le $((750 * 1024)) ]] || error "initial Brotli payload exceeds 750 KiB: $INITIAL_BROTLI_BYTES"

capture_host() {
  local pid="$1"
  for _ in $(seq 1 50); do
    [[ -r "/proc/$pid/stat" && -r "/proc/$pid/cmdline" ]] && break
    sleep 0.1
  done
  HOST_STARTTIME="$(process_start "$pid")"
  HOST_CMDLINE="$(tr '\0' ' ' <"/proc/$pid/cmdline" | sed 's/[[:space:]]*$//')"
  local live_exe
  live_exe="$(readlink -e -- "/proc/$pid/exe")"
  [[ "$live_exe" == "$KANBAN" ]] || error "host /proc/exe differs from built binary: $live_exe"
  local -a live_argv=()
  while IFS= read -r -d '' argument; do live_argv+=("$argument"); done <"/proc/$pid/cmdline"
  local -a expected_argv=(
    "$KANBAN" "--db" "$DB_PATH" "serve" "--host" "127.0.0.1" "--port" "$PORT" "--web-dir" "$ROOT/apps/web/dist"
  )
  [[ "${#live_argv[@]}" -eq "${#expected_argv[@]}" ]] || error "host argv token count mismatch"
  for index in "${!expected_argv[@]}"; do
    [[ "${live_argv[$index]}" == "${expected_argv[$index]}" ]] || error "host argv token mismatch at $index"
  done
  HOST_ARGV_JSON="$(printf '%s\n' "${live_argv[@]}" | jq -R -s 'split("\n") | map(select(length > 0) ) | @json')"
  HOST_ARGV_JSON="$(jq -r . <<<"$HOST_ARGV_JSON")"
  local live_binary_sha
  live_binary_sha="sha256:$(sha256sum -- "/proc/$pid/exe" | awk '{print $1}')"
  [[ -n "$HOST_STARTTIME" && "$HOST_CMDLINE" == *kanban* && "$HOST_CMDLINE" == *serve* ]] || error "host PID is not kanban serve"
  [[ "$live_binary_sha" == "$KANBAN_BINARY_SHA256" ]] || error "live host executable differs from built binary"
}
listener_owners() {
  ss -H -ltnp 2>/dev/null | awk -v port=":$PORT" '
    ($4 ~ port "$" || $4 ~ /\]:/ port "$") {
      line = $0
      while (match(line, /pid=[0-9]+/)) {
        value = substr(line, RSTART + 4, RLENGTH - 4)
        print value
        line = substr(line, RSTART + RLENGTH)
      }
    }
  ' | sort -u
}
wait_listener_bound() {
  local owners
  for _ in $(seq 1 100); do
    owners="$(listener_owners)"
    if [[ "$owners" == "$HOST_PID" ]]; then return 0; fi
    sleep 0.1
  done
  error "listener owner set is not exactly host PID: expected=$HOST_PID actual=$(listener_owners | tr '\n' ' ')"
}
start_host() {
  setsid env KANBAN_ACTOR=release-09d "$KANBAN" --db "$DB_PATH" serve --host 127.0.0.1 --port "$PORT" --web-dir "$ROOT/apps/web/dist" >>"$HOST_LOG" 2>&1 &
  HOST_PID="$!"
  capture_host "$HOST_PID"
}
start_host
wait_listener_bound
curl --fail --silent --show-error --retry 120 --retry-all-errors --retry-delay 1 "$BASE_URL/health" >/dev/null

HEALTH_BEFORE="$(curl --fail --silent --show-error "$BASE_URL/health")"
if ! jq -e --arg db_path "$DB_PATH" '.data.ok == true and .data.db == "turso" and .data.db_path == $db_path' <<<"$HEALTH_BEFORE" >/dev/null; then
  error "initial health identity does not match Turso DB path"
fi

BOARDS_JSON="$(KANBAN_SERVER_URL="$BASE_URL" "$KANBAN" --json --board default board list)"
if ! jq -e '.data | any(.slug == "default")' <<<"$BOARDS_JSON" >/dev/null; then
  KANBAN_SERVER_URL="$BASE_URL" "$KANBAN" --json --board default board create default --name "Stage09 09D" >/dev/null
fi
KANBAN_SERVER_URL="$BASE_URL" "$KANBAN" --json --board default task create "Stage09 09D seed" --status todo --task-id t_release_09d_seed >/dev/null

HEALTH_DB_PATH="$(jq -er '.data.db_path' <<<"$HEALTH_BEFORE")"
[[ "$HEALTH_DB_PATH" == "$DB_PATH" ]] || error "health db_path differs from launcher DB"
[[ -f "$HEALTH_DB_PATH" && ! -L "$HEALTH_DB_PATH" ]] || error "canonical DB is not a regular file"
DB_IDENTITY_BEFORE="$(stat -c '%d:%i' -- "$HEALTH_DB_PATH")"
HOST_PID_RECORDED="$HOST_PID"
HOST_STARTTIME_RECORDED="$HOST_STARTTIME"

export KANBAN_RELEASE_09D_BASE_URL="$BASE_URL"
export KANBAN_RELEASE_09D_RUN_ID="$RUN_ID"
export KANBAN_RELEASE_09D_FORMAL="$FORMAL_MODE"
export KANBAN_RELEASE_09D_EVIDENCE_DIR="$EVIDENCE_DIR"
export KANBAN_RELEASE_09D_PORT="$PORT"
export KANBAN_RELEASE_09D_START_SHA="$START_SHA"
export KANBAN_RELEASE_09D_START_CLEAN="$START_CLEAN"
export KANBAN_RELEASE_09D_HOST_PID="$HOST_PID"
export KANBAN_RELEASE_09D_HOST_STARTTIME="$HOST_STARTTIME"
export KANBAN_RELEASE_09D_HOST_ARGV="$HOST_CMDLINE"
export KANBAN_RELEASE_09D_HOST_ARGV_JSON="$HOST_ARGV_JSON"
export KANBAN_RELEASE_09D_BINARY_SHA256="$KANBAN_BINARY_SHA256"
export KANBAN_RELEASE_09D_ARTIFACT_BUILD_ID="$ARTIFACT_BUILD_ID"
export KANBAN_RELEASE_09D_ARTIFACT_MANIFEST_SHA256="$ARTIFACT_MANIFEST_SHA256"
export KANBAN_RELEASE_09D_DB_PATH="$DB_PATH"
export KANBAN_RELEASE_09D_DB_IDENTITY_BEFORE="$DB_IDENTITY_BEFORE"
export KANBAN_RELEASE_09D_INITIAL_BROTLI_BYTES="$INITIAL_BROTLI_BYTES"
export KANBAN_RELEASE_09D_INITIAL_BROTLI_FILES="$INITIAL_FILES_JSON"

run_playwright() {
  local current_phase="$1" project="$2" grep_pattern="$3"
  KANBAN_RELEASE_09D_PHASE="$current_phase" \
    pnpm --filter @kanban-tool/web exec playwright test \
      --config=playwright.release-09d.config.ts \
      --project="$project" \
      --grep "$grep_pattern"
}

# 整条有序链路复用同一个 canonical DB：依次执行小数据性能/SSE、精确 2k 功能验证，
# 再执行精确 5k UI 压力验证；Firefox 只覆盖关键路径。
run_playwright "small-perf-sse" chromium "09D (performance|persistent SSE)"
run_playwright "small-sse" firefox "09D persistent SSE"
run_playwright "functional-2k" chromium "09D functional_2k real board and list pagination"
run_playwright "functional-2k" firefox "09D functional_2k real board and list pagination"
run_playwright "stress-5k" chromium "09D stress_5k real board list map no-crash bounded interaction"
run_playwright "stress-5k" firefox "09D stress_5k real board list map no-crash bounded interaction"

HEALTH_AFTER="$(curl --fail --silent --show-error "$BASE_URL/health")"
HEALTH_DB_PATH_AFTER="$(jq -er '.data.db_path' <<<"$HEALTH_AFTER")"
[[ "$HEALTH_DB_PATH_AFTER" == "$DB_PATH" ]] || error "health db_path changed during 09D"
DB_IDENTITY_AFTER="$(stat -c '%d:%i' -- "$HEALTH_DB_PATH_AFTER")"
[[ "$DB_IDENTITY_AFTER" == "$DB_IDENTITY_BEFORE" ]] || error "canonical DB inode changed during 09D"
DB_FINGERPRINT_AFTER="$(jq -er '.data.db_fingerprint' <<<"$HEALTH_AFTER")"
FINAL_TOTAL="$(curl --fail --silent --show-error "$BASE_URL/api/v1/boards/default/tasks?limit=1&offset=0&sort=seq" | jq -er '.meta.total')"
[[ "$FINAL_TOTAL" == "5000" ]] || error "final exact task total is $FINAL_TOTAL, expected 5000"

stop_host
if curl --silent --max-time 0.5 "$BASE_URL/health" >/dev/null 2>&1; then error "09D host port remained open after stop"; fi
[[ -z "$(port_listeners)" ]] || error "09D host port listener remained after stop"
[[ -z "$(listener_owners)" ]] || error "09D listener remained after stop: $(listener_owners | tr '\n' ' ')"
if [[ -e "/proc/$HOST_PID_RECORDED" ]] && process_alive "$HOST_PID_RECORDED" "$HOST_STARTTIME_RECORDED"; then
  error "09D host PID remained alive after stop: $HOST_PID_RECORDED"
fi

check_private_file() {
  local file="$1"
  [[ -f "$file" && ! -L "$file" ]] || error "09D evidence is not a regular non-symlink file: $file"
  [[ "$(stat -c '%h' -- "$file")" == "1" ]] || error "09D evidence hard-link count is not 1: $file"
  [[ "$(stat -c '%a' -- "$file")" == "600" ]] || error "09D evidence file must be mode 600: $file"
}

for evidence in \
  "$EVIDENCE_DIR/release-09d-small-perf-sse-chromium.json" \
  "$EVIDENCE_DIR/release-09d-small-sse-firefox.json" \
  "$EVIDENCE_DIR/release-09d-functional-2k-chromium.json" \
  "$EVIDENCE_DIR/release-09d-functional-2k-firefox.json" \
  "$EVIDENCE_DIR/release-09d-stress-5k-chromium.json" \
  "$EVIDENCE_DIR/release-09d-stress-5k-firefox.json"; do
  check_private_file "$evidence"
done
if find "$EVIDENCE_DIR" -maxdepth 1 -type f \( -name '.release-09d-evidence.tmp' -o -name 'release-09d-*.tmp' \) -print -quit | grep -q .; then
  error "stale 09D evidence temporary file remains"
fi

END_SHA="$(git -C "$ROOT" rev-parse HEAD)"
END_CLEAN=false
if [[ -z "$(git -C "$ROOT" status --porcelain=v1)" ]]; then END_CLEAN=true; fi
[[ "$END_SHA" == "$START_SHA" ]] || error "09D HEAD changed during proof: start=$START_SHA end=$END_SHA"
if [[ "$FORMAL_MODE" == "true" && ( "$START_CLEAN" != "true" || "$END_CLEAN" != "true" ) ]]; then
  error "formal 09D proof requires clean worktree at start and end; use KANBAN_RELEASE_09D_FORMAL=false for dirty diagnostics"
fi

check_browser_evidence() {
  local file="$1" browser="$2" performance_gate="$3" sse_gate="$4" functional_gate="$5" stress_gate="$6" metric_samples="$7" sse_samples="$8" task_target="$9" final_total="${10}" latency_status="${11}" key_path_status="${12}" map_required="${13}" stream_min="${14}" reconnect_required="${15}" ui_kind="${16}"
  jq -e \
    --arg browser "$browser" \
    --arg performance_gate "$performance_gate" \
    --arg sse_gate "$sse_gate" \
    --arg functional_gate "$functional_gate" \
    --arg stress_gate "$stress_gate" \
    --arg latency_status "$latency_status" \
    --arg key_path_status "$key_path_status" \
    --arg formal "$FORMAL_MODE" \
    --arg run_id "$RUN_ID" \
    --arg base_url "$BASE_URL" \
    --arg start_sha "$START_SHA" \
    --arg host_starttime "$HOST_STARTTIME_RECORDED" \
    --argjson host_argv_tokens "$HOST_ARGV_JSON" \
    --arg binary_sha "$KANBAN_BINARY_SHA256" \
    --arg artifact_build_id "$ARTIFACT_BUILD_ID" \
    --arg artifact_manifest_sha "$ARTIFACT_MANIFEST_SHA256" \
    --arg db_path "$DB_PATH" \
    --arg db_before "$DB_IDENTITY_BEFORE" \
    --argjson host_pid "$HOST_PID_RECORDED" \
    --argjson metric_samples "$metric_samples" \
    --argjson sse_samples "$sse_samples" \
    --argjson task_target "$task_target" \
    --argjson final_total "$final_total" \
    --argjson map_required "$map_required" \
    --argjson stream_min "$stream_min" \
    --argjson reconnect_required "$reconnect_required" \
    --arg ui_kind "$ui_kind" \
    '(
      .browser == $browser
      and .run_id == $run_id
      and .base_url == $base_url
      and .start_sha == $start_sha
      and .formal == ($formal == "true")
      and .host_pid == $host_pid
      and .host_starttime == $host_starttime
      and .host_argv_tokens == $host_argv_tokens
      and .binary_sha256 == $binary_sha
      and .artifact_build_id == $artifact_build_id
      and .artifact_manifest_sha256 == $artifact_manifest_sha
      and .db_path == $db_path
      and .db_identity_before == $db_before
      and .gates.performance == $performance_gate
      and .gates.sse == $sse_gate
      and .gates.functional_2k == $functional_gate
      and .gates.stress_5k == $stress_gate
      and .sse_contract.latency_budget_status == $latency_status
      and .sse_contract.key_path_status == $key_path_status
      and ((.performance.samples // []) | length) == $metric_samples
      and ((.sse.samples // []) | length) == $sse_samples
      and (.browser_errors | length) == 0
      and .failure == null
      and (.fixture_seed.response_error_count // 0) == 0
      and (if $task_target == 0
        then ((.task_counts // []) | length) == 0
        else ((.task_counts // []) | length) == 1 and .task_counts[0].target == $task_target and .task_counts[0].total == $task_target
        end)
      and (if $task_target == 0
        then ((.fixture_seed.records // []) | length) == 0
        else ((.fixture_seed.records // []) | length) == 1
          and .fixture_seed.records[0].target == $task_target
          and .fixture_seed.records[0].requested == .fixture_seed.records[0].created
          and .fixture_seed.records[0].final_total == $task_target
          and .fixture_seed.records[0].duration_ms >= 0
          and .fixture_seed.records[0].concurrency == 16
        end)
      and (if $final_total == null then true else .final_task_total == $final_total end)
      and (if $map_required == 1 then .map != null and .map.node_count > 0 and .map.node_count <= .map.limit_nodes and .map.limit_nodes == 240 and (.map.truncated | type) == "boolean" and .map.ui_node_count == .map.node_count and .map.ui_edge_count == .map.edge_count and .map.ui_truncated == .map.truncated and .map.zoom_before == 100 and .map.zoom_after == 115 else .map == null end)
      and (if $ui_kind == "none"
        then .ui == null
        elif $ui_kind == "functional_2k"
        then .ui != null and .ui.functional_2k != null and .ui.stress_5k == null
          and .ui.functional_2k.board_total == 2000
          and .ui.functional_2k.board_rendered_count > 0
          and .ui.functional_2k.board_rendered_count <= ((.ui.functional_2k.board_columns | length) * 100)
          and .ui.functional_2k.board_rendered_count == ([.ui.functional_2k.board_columns[].rendered_card_count] | add // 0)
          and ([.ui.functional_2k.board_columns[].total] | add // 0) == 2000
          and ([.ui.functional_2k.board_columns[] | select(.rendered_card_count > 100 or .total != .range_total or .range_start > .range_end or .range_end > .total or .page != 1 or .total_pages < 1 or (.total == 0 and (.range_start != 0 or .range_end != 0 or .rendered_card_count != 0)) or (.total > 0 and (.range_start != 1 or .rendered_card_count != (.range_end - .range_start + 1))))] | length) == 0
          and .ui.functional_2k.board_ready.budget_ms == 120000
          and .ui.functional_2k.board_ready.within_budget == true
          and (.ui.functional_2k.board_ready.navigation_start_ms | type) == "number"
          and (.ui.functional_2k.board_ready.ready_ms | type) == "number"
          and .ui.functional_2k.board_ready.ready_ms <= .ui.functional_2k.board_ready.budget_ms
          and .ui.functional_2k.board_window.page_size == 100
          and .ui.functional_2k.board_window.page1.page == 1
          and .ui.functional_2k.board_window.page1.range_start == 1
          and .ui.functional_2k.board_window.page1.range_end == 100
          and .ui.functional_2k.board_window.page1.total == .ui.functional_2k.board_window.page2.total
          and .ui.functional_2k.board_window.page1.total >= 101
          and ((.ui.functional_2k.board_window.column_id) as $column_id | (.ui.functional_2k.board_window.page1.total) as $window_total | ([.ui.functional_2k.board_columns[] | select(.column_id == $column_id and .total == $window_total and .total_pages >= 2 and .rendered_card_count <= 100)] | length) == 1)
          and .ui.functional_2k.board_window.page1.first_task_id != null
          and .ui.functional_2k.board_window.page1.last_task_id != null
          and .ui.functional_2k.board_window.page2.page == 2
          and .ui.functional_2k.board_window.page2.range_start == 101
          and .ui.functional_2k.board_window.page2.range_end == 200
          and .ui.functional_2k.board_window.page2.total == .ui.functional_2k.board_window.page1.total
          and .ui.functional_2k.board_window.page2.first_task_id != null
          and .ui.functional_2k.board_window.page2.last_task_id != null
          and .ui.functional_2k.board_window.page2.first_task_id != .ui.functional_2k.board_window.page1.first_task_id
          and .ui.functional_2k.board_window.page2.last_task_id != .ui.functional_2k.board_window.page1.last_task_id
          and .ui.functional_2k.list_total == 2000
          and .ui.functional_2k.page1.row_count == 100
          and .ui.functional_2k.page1.range == "1–100 / 2000"
          and .ui.functional_2k.page1.first_task_id != null
          and .ui.functional_2k.page1.last_task_id != null
          and .ui.functional_2k.page2.row_count == 100
          and .ui.functional_2k.page2.range == "101–200 / 2000"
          and .ui.functional_2k.page2.first_task_id != null
          and .ui.functional_2k.page2.last_task_id != null
        elif $ui_kind == "stress_5k"
        then .ui != null and .ui.functional_2k == null and .ui.stress_5k != null
          and .ui.stress_5k.board_total == 5000
          and .ui.stress_5k.board_rendered_count > 0
          and .ui.stress_5k.board_rendered_count <= ((.ui.stress_5k.board_columns | length) * 100)
          and .ui.stress_5k.board_rendered_count == ([.ui.stress_5k.board_columns[].rendered_card_count] | add // 0)
          and ([.ui.stress_5k.board_columns[].total] | add // 0) == 5000
          and ([.ui.stress_5k.board_columns[] | select(.rendered_card_count > 100 or .total != .range_total or .range_start > .range_end or .range_end > .total or .page != 1 or .total_pages < 1 or (.total == 0 and (.range_start != 0 or .range_end != 0 or .rendered_card_count != 0)) or (.total > 0 and (.range_start != 1 or .rendered_card_count != (.range_end - .range_start + 1))))] | length) == 0
          and .ui.stress_5k.board_ready.budget_ms == 120000
          and .ui.stress_5k.board_ready.within_budget == true
          and (.ui.stress_5k.board_ready.navigation_start_ms | type) == "number"
          and (.ui.stress_5k.board_ready.ready_ms | type) == "number"
          and .ui.stress_5k.board_ready.ready_ms <= .ui.stress_5k.board_ready.budget_ms
          and .ui.stress_5k.board_window.page_size == 100
          and .ui.stress_5k.board_window.page1.page == 1
          and .ui.stress_5k.board_window.page1.range_start == 1
          and .ui.stress_5k.board_window.page1.range_end == 100
          and .ui.stress_5k.board_window.page1.total == .ui.stress_5k.board_window.page2.total
          and .ui.stress_5k.board_window.page1.total >= 101
          and ((.ui.stress_5k.board_window.column_id) as $column_id | (.ui.stress_5k.board_window.page1.total) as $window_total | ([.ui.stress_5k.board_columns[] | select(.column_id == $column_id and .total == $window_total and .total_pages >= 2 and .rendered_card_count <= 100)] | length) == 1)
          and .ui.stress_5k.board_window.page1.first_task_id != null
          and .ui.stress_5k.board_window.page1.last_task_id != null
          and .ui.stress_5k.board_window.page2.page == 2
          and .ui.stress_5k.board_window.page2.range_start == 101
          and .ui.stress_5k.board_window.page2.range_end == 200
          and .ui.stress_5k.board_window.page2.total == .ui.stress_5k.board_window.page1.total
          and .ui.stress_5k.board_window.page2.first_task_id != null
          and .ui.stress_5k.board_window.page2.last_task_id != null
          and .ui.stress_5k.board_window.page2.first_task_id != .ui.stress_5k.board_window.page1.first_task_id
          and .ui.stress_5k.board_window.page2.last_task_id != .ui.stress_5k.board_window.page1.last_task_id
          and .ui.stress_5k.list_total == 5000
          and .ui.stress_5k.before_limit == 100
          and .ui.stress_5k.before.row_count == 100
          and .ui.stress_5k.before.range == "1–100 / 5000"
          and .ui.stress_5k.after_limit == 200
          and .ui.stress_5k.after.row_count == 200
          and .ui.stress_5k.after.range == "1–200 / 5000"
          and .ui.stress_5k.map.node_count == .map.node_count
          and .ui.stress_5k.map.edge_count == .map.edge_count
          and .ui.stress_5k.map.truncated == .map.truncated
          and .ui.stress_5k.map.limit_nodes == 240
          and .ui.stress_5k.map.zoom_before == 100
          and .ui.stress_5k.map.zoom_after == 115
        else false
        end)
      and ((.sse_stream_requests // []) | length) >= $stream_min
      and .sse_reconnect.new_request_after_disconnect == ($reconnect_required == 1)
      and .sse_reconnect.stale_notice_cleared == ($reconnect_required == 1)
      and (if $reconnect_required == 1
        then .sse_reconnect.request_count_after_disconnect > .sse_reconnect.before_count
          and (.sse_reconnect.request_at_ms != null)
          and (.sse_reconnect.event_seen_at_ms != null)
          and .sse_reconnect.request_at_ms <= .sse_reconnect.event_seen_at_ms
          and (.sse_reconnect.last_event_id != null)
          and (.sse_reconnect.after != null)
          and (.sse_reconnect.confirmed_cursor != null)
          and (.sse_reconnect.confirmed_task_id != null)
          and (.sse_reconnect.confirmed_event_id != null)
          and .sse_reconnect.last_event_id == .sse_reconnect.after
          and .sse_reconnect.after == .sse_reconnect.confirmed_cursor
        else .sse_reconnect.request_count_after_disconnect == 0
          and .sse_reconnect.request_at_ms == null
          and .sse_reconnect.event_seen_at_ms == null
          and .sse_reconnect.last_event_id == null
          and .sse_reconnect.after == null
          and .sse_reconnect.confirmed_cursor == null
          and .sse_reconnect.confirmed_task_id == null
          and .sse_reconnect.confirmed_event_id == null
        end)
    )' \
    -- "$file" >/dev/null || error "browser evidence contract failed: $file"
}

check_browser_evidence "$EVIDENCE_DIR/release-09d-small-perf-sse-chromium.json" chromium passed passed not-run not-run 20 21 0 null passed passed 0 2 1 none
check_browser_evidence "$EVIDENCE_DIR/release-09d-small-sse-firefox.json" firefox not-run passed not-run not-run 0 2 0 null not_applicable passed 0 2 1 none
check_browser_evidence "$EVIDENCE_DIR/release-09d-functional-2k-chromium.json" chromium not-run not-run passed not-run 0 0 2000 2000 not_run not_run 0 0 0 functional_2k
check_browser_evidence "$EVIDENCE_DIR/release-09d-functional-2k-firefox.json" firefox not-run not-run passed not-run 0 0 2000 2000 not_run not_run 0 0 0 functional_2k
check_browser_evidence "$EVIDENCE_DIR/release-09d-stress-5k-chromium.json" chromium not-run not-run not-run passed 0 0 5000 5000 not_run not_run 1 0 0 stress_5k
check_browser_evidence "$EVIDENCE_DIR/release-09d-stress-5k-firefox.json" firefox not-run not-run not-run passed 0 0 5000 5000 not_run not_run 1 0 0 stress_5k

HOST_LOG_EVIDENCE_PATH="$EVIDENCE_DIR/host-${RUN_ID}.log"
node "$ROOT/scripts/release-proof-09d.mjs" write-json --path "$HOST_LOG_EVIDENCE_PATH" <"$HOST_LOG"
check_private_file "$HOST_LOG_EVIDENCE_PATH"
HOST_LOG_RELATIVE_PATH="output/release/09d/host-${RUN_ID}.log"
HOST_LOG_SHA256="sha256:$(sha256sum -- "$HOST_LOG_EVIDENCE_PATH" | awk '{print $1}')"
rm -rf -- "$TMP_ROOT"
TMP_ROOT=""
if [[ -e "$DB_PATH" ]]; then error "temporary canonical DB was not removed"; fi
if [[ -e "$TMP_ROOT_RECORDED" ]]; then error "temporary canonical root was not removed"; fi
DB_REMOVED=true
INITIAL_FILES_JSON="$INITIAL_FILES_JSON" \
  jq -n \
  --slurpfile a "$EVIDENCE_DIR/release-09d-small-perf-sse-chromium.json" \
  --slurpfile b "$EVIDENCE_DIR/release-09d-small-sse-firefox.json" \
  --slurpfile c "$EVIDENCE_DIR/release-09d-functional-2k-chromium.json" \
  --slurpfile d "$EVIDENCE_DIR/release-09d-functional-2k-firefox.json" \
  --slurpfile e "$EVIDENCE_DIR/release-09d-stress-5k-chromium.json" \
  --slurpfile f "$EVIDENCE_DIR/release-09d-stress-5k-firefox.json" \
  --arg run_id "$RUN_ID" \
  --arg formal "$FORMAL_MODE" \
  --arg base_url "$BASE_URL" \
  --arg start_sha "$START_SHA" \
  --arg start_clean "$START_CLEAN" \
  --arg end_sha "$END_SHA" \
  --arg end_clean "$END_CLEAN" \
  --arg host_starttime "$HOST_STARTTIME_RECORDED" \
  --arg binary_sha "$KANBAN_BINARY_SHA256" \
  --arg artifact_build_id "$ARTIFACT_BUILD_ID" \
  --arg artifact_manifest_sha "$ARTIFACT_MANIFEST_SHA256" \
  --arg db_path "$DB_PATH" \
  --arg db_before "$DB_IDENTITY_BEFORE" \
  --arg db_after "$DB_IDENTITY_AFTER" \
  --arg db_fingerprint "$DB_FINGERPRINT_AFTER" \
  --arg host_log_sha "$HOST_LOG_SHA256" \
  --arg host_log_path "$HOST_LOG_RELATIVE_PATH" \
  --argjson host_pid "$(jq -n --arg value "${KANBAN_RELEASE_09D_HOST_PID:-0}" '$value | tonumber')" \
  --argjson port "$PORT" \
  --argjson final_total "$FINAL_TOTAL" \
  --argjson initial_brotli "$INITIAL_BROTLI_BYTES" \
  --argjson initial_files "$INITIAL_FILES_JSON" \
  --argjson db_removed "$DB_REMOVED" \
  ' {
      schema_version: 1,
      stage: "stage09-release-proof-09D",
      run_id: $run_id,
      formal: ($formal == "true"),
      release_status: (if $formal == "true" then "passed" else "diagnostic" end),
      host: {
        base_url: $base_url,
        port: $port,
        start_sha: $start_sha,
        start_clean: ($start_clean == "true"),
        end_sha: $end_sha,
        end_clean: ($end_clean == "true"),
        binary_sha256: $binary_sha,
        artifact_build_id: $artifact_build_id,
        artifact_manifest_sha256: $artifact_manifest_sha,
        pid: $host_pid,
        starttime: $host_starttime,
        db_path: $db_path,
        db_identity_before: $db_before,
        db_identity_after: $db_after,
        db_fingerprint_after: $db_fingerprint,
        host_log_sha256: $host_log_sha,
        host_log_path: $host_log_path,
        live_binary_bound: true
      },
      initial_brotli: { bytes: $initial_brotli, budget_bytes: (750 * 1024), files: $initial_files },
      fixtures: { exact_2k: true, exact_5k: ($final_total == 5000), final_task_total: $final_total },
      browsers: ($a + $b + $c + $d + $e + $f),
      cleanup: { host_stopped: true, port_free: true, db_removed: $db_removed, temp_root_removed: true, evidence_directory: "output/release/09d" }
    }' | node "$ROOT/scripts/release-proof-09d.mjs" write-json --path "$EVIDENCE_DIR/release-09d-evidence.json"
check_private_file "$EVIDENCE_DIR/release-09d-evidence.json"

EXPECTED_RELEASE_STATUS="diagnostic"
if [[ "$FORMAL_MODE" == "true" ]]; then EXPECTED_RELEASE_STATUS="passed"; fi
jq -e --arg expected_status "$EXPECTED_RELEASE_STATUS" '.release_status == $expected_status and .formal == ($expected_status == "passed") and .fixtures.exact_2k and .fixtures.exact_5k and .cleanup.host_stopped and .cleanup.port_free and .cleanup.db_removed and .cleanup.temp_root_removed and .initial_brotli.bytes <= .initial_brotli.budget_bytes' "$EVIDENCE_DIR/release-09d-evidence.json" >/dev/null
jq -e --arg host_log_path "$HOST_LOG_RELATIVE_PATH" --arg host_log_sha "$HOST_LOG_SHA256" --arg start_sha "$START_SHA" --arg end_sha "$END_SHA" --arg host_starttime "$HOST_STARTTIME_RECORDED" --argjson host_pid "$HOST_PID_RECORDED" '.host.host_log_path == $host_log_path and .host.host_log_sha256 == $host_log_sha and .host.start_sha == $start_sha and .host.end_sha == $end_sha and .host.pid == $host_pid and .host.starttime == $host_starttime and .host.live_binary_bound == true' "$EVIDENCE_DIR/release-09d-evidence.json" >/dev/null
jq -e '
  def ui_ok($kind):
    if $kind == "none" then .ui == null
    elif $kind == "functional_2k"
    then .ui != null and .ui.functional_2k != null and .ui.stress_5k == null
      and .ui.functional_2k.board_total == 2000
      and .ui.functional_2k.board_rendered_count > 0
      and .ui.functional_2k.board_rendered_count <= ((.ui.functional_2k.board_columns | length) * 100)
      and .ui.functional_2k.board_rendered_count == ([.ui.functional_2k.board_columns[].rendered_card_count] | add // 0)
      and ([.ui.functional_2k.board_columns[].total] | add // 0) == 2000
      and ([.ui.functional_2k.board_columns[] | select(.rendered_card_count > 100 or .total != .range_total or .range_start > .range_end or .range_end > .total or .page != 1 or .total_pages < 1 or (.total == 0 and (.range_start != 0 or .range_end != 0 or .rendered_card_count != 0)) or (.total > 0 and (.range_start != 1 or .rendered_card_count != (.range_end - .range_start + 1))))] | length) == 0
      and .ui.functional_2k.board_ready.budget_ms == 120000
      and .ui.functional_2k.board_ready.within_budget == true
      and (.ui.functional_2k.board_ready.navigation_start_ms | type) == "number"
      and (.ui.functional_2k.board_ready.ready_ms | type) == "number"
      and .ui.functional_2k.board_ready.ready_ms <= .ui.functional_2k.board_ready.budget_ms
      and .ui.functional_2k.board_window.page_size == 100
      and .ui.functional_2k.board_window.page1.page == 1
      and .ui.functional_2k.board_window.page1.range_start == 1
      and .ui.functional_2k.board_window.page1.range_end == 100
      and .ui.functional_2k.board_window.page1.total == .ui.functional_2k.board_window.page2.total
      and .ui.functional_2k.board_window.page1.total >= 101
      and ((.ui.functional_2k.board_window.column_id) as $column_id | (.ui.functional_2k.board_window.page1.total) as $window_total | ([.ui.functional_2k.board_columns[] | select(.column_id == $column_id and .total == $window_total and .total_pages >= 2 and .rendered_card_count <= 100)] | length) == 1)
      and .ui.functional_2k.board_window.page1.first_task_id != null
      and .ui.functional_2k.board_window.page1.last_task_id != null
      and .ui.functional_2k.board_window.page2.page == 2
      and .ui.functional_2k.board_window.page2.range_start == 101
      and .ui.functional_2k.board_window.page2.range_end == 200
      and .ui.functional_2k.board_window.page2.total == .ui.functional_2k.board_window.page1.total
      and .ui.functional_2k.board_window.page2.first_task_id != null
      and .ui.functional_2k.board_window.page2.last_task_id != null
      and .ui.functional_2k.board_window.page2.first_task_id != .ui.functional_2k.board_window.page1.first_task_id
      and .ui.functional_2k.board_window.page2.last_task_id != .ui.functional_2k.board_window.page1.last_task_id
      and .ui.functional_2k.list_total == 2000
      and .ui.functional_2k.page1.row_count == 100
      and .ui.functional_2k.page1.range == "1–100 / 2000"
      and .ui.functional_2k.page1.first_task_id != null
      and .ui.functional_2k.page1.last_task_id != null
      and .ui.functional_2k.page2.row_count == 100
      and .ui.functional_2k.page2.range == "101–200 / 2000"
      and .ui.functional_2k.page2.first_task_id != null
      and .ui.functional_2k.page2.last_task_id != null
    elif $kind == "stress_5k"
    then .ui != null and .ui.functional_2k == null and .ui.stress_5k != null
      and .ui.stress_5k.board_total == 5000
      and .ui.stress_5k.board_rendered_count > 0
      and .ui.stress_5k.board_rendered_count <= ((.ui.stress_5k.board_columns | length) * 100)
      and .ui.stress_5k.board_rendered_count == ([.ui.stress_5k.board_columns[].rendered_card_count] | add // 0)
      and ([.ui.stress_5k.board_columns[].total] | add // 0) == 5000
      and ([.ui.stress_5k.board_columns[] | select(.rendered_card_count > 100 or .total != .range_total or .range_start > .range_end or .range_end > .total or .page != 1 or .total_pages < 1 or (.total == 0 and (.range_start != 0 or .range_end != 0 or .rendered_card_count != 0)) or (.total > 0 and (.range_start != 1 or .rendered_card_count != (.range_end - .range_start + 1))))] | length) == 0
      and .ui.stress_5k.board_ready.budget_ms == 120000
      and .ui.stress_5k.board_ready.within_budget == true
      and (.ui.stress_5k.board_ready.navigation_start_ms | type) == "number"
      and (.ui.stress_5k.board_ready.ready_ms | type) == "number"
      and .ui.stress_5k.board_ready.ready_ms <= .ui.stress_5k.board_ready.budget_ms
      and .ui.stress_5k.board_window.page_size == 100
      and .ui.stress_5k.board_window.page1.page == 1
      and .ui.stress_5k.board_window.page1.range_start == 1
      and .ui.stress_5k.board_window.page1.range_end == 100
      and .ui.stress_5k.board_window.page1.total == .ui.stress_5k.board_window.page2.total
      and .ui.stress_5k.board_window.page1.total >= 101
      and ((.ui.stress_5k.board_window.column_id) as $column_id | (.ui.stress_5k.board_window.page1.total) as $window_total | ([.ui.stress_5k.board_columns[] | select(.column_id == $column_id and .total == $window_total and .total_pages >= 2 and .rendered_card_count <= 100)] | length) == 1)
      and .ui.stress_5k.board_window.page1.first_task_id != null
      and .ui.stress_5k.board_window.page1.last_task_id != null
      and .ui.stress_5k.board_window.page2.page == 2
      and .ui.stress_5k.board_window.page2.range_start == 101
      and .ui.stress_5k.board_window.page2.range_end == 200
      and .ui.stress_5k.board_window.page2.total == .ui.stress_5k.board_window.page1.total
      and .ui.stress_5k.board_window.page2.first_task_id != null
      and .ui.stress_5k.board_window.page2.last_task_id != null
      and .ui.stress_5k.board_window.page2.first_task_id != .ui.stress_5k.board_window.page1.first_task_id
      and .ui.stress_5k.board_window.page2.last_task_id != .ui.stress_5k.board_window.page1.last_task_id
      and .ui.stress_5k.list_total == 5000
      and .ui.stress_5k.before_limit == 100
      and .ui.stress_5k.before.row_count == 100
      and .ui.stress_5k.before.range == "1–100 / 5000"
      and .ui.stress_5k.after_limit == 200
      and .ui.stress_5k.after.row_count == 200
      and .ui.stress_5k.after.range == "1–200 / 5000"
      and .ui.stress_5k.map.node_count == .map.node_count
      and .ui.stress_5k.map.edge_count == .map.edge_count
      and .ui.stress_5k.map.truncated == .map.truncated
      and .ui.stress_5k.map.limit_nodes == 240
      and .ui.stress_5k.map.zoom_before == 100
      and .ui.stress_5k.map.zoom_after == 115
      and .map.ui_node_count == .map.node_count
      and .map.ui_edge_count == .map.edge_count
      and .map.ui_truncated == .map.truncated
      and .map.zoom_before == 100
      and .map.zoom_after == 115
    else false end;
  def record($phase; $browser; $performance; $sse; $functional; $stress; $metrics; $events; $target; $latency; $key_path; $ui):
    .phase == $phase
    and .browser == $browser
    and .gates.performance == $performance
    and .gates.sse == $sse
    and .gates.functional_2k == $functional
    and .gates.stress_5k == $stress
    and .sse_contract.latency_budget_status == $latency
    and .sse_contract.key_path_status == $key_path
    and ((.performance.samples // []) | length) == $metrics
    and ((.sse.samples // []) | length) == $events
    and (.browser_errors | length) == 0
    and .failure == null
    and (if $target == 0 then ((.task_counts // []) | length) == 0 else ((.task_counts // []) | length) == 1 and .task_counts[0].target == $target and .task_counts[0].total == $target end)
    and ui_ok($ui);
  (.browsers | length) == 6
  and ([.browsers[] | select(record("small-perf-sse"; "chromium"; "passed"; "passed"; "not-run"; "not-run"; 20; 21; 0; "passed"; "passed"; "none"))] | length) == 1
  and ([.browsers[] | select(record("small-sse"; "firefox"; "not-run"; "passed"; "not-run"; "not-run"; 0; 2; 0; "not_applicable"; "passed"; "none"))] | length) == 1
  and ([.browsers[] | select(record("functional-2k"; "chromium"; "not-run"; "not-run"; "passed"; "not-run"; 0; 0; 2000; "not_run"; "not_run"; "functional_2k"))] | length) == 1
  and ([.browsers[] | select(record("functional-2k"; "firefox"; "not-run"; "not-run"; "passed"; "not-run"; 0; 0; 2000; "not_run"; "not_run"; "functional_2k"))] | length) == 1
  and ([.browsers[] | select(record("stress-5k"; "chromium"; "not-run"; "not-run"; "not-run"; "passed"; 0; 0; 5000; "not_run"; "not_run"; "stress_5k"))] | length) == 1
  and ([.browsers[] | select(record("stress-5k"; "firefox"; "not-run"; "not-run"; "not-run"; "passed"; 0; 0; 5000; "not_run"; "not_run"; "stress_5k"))] | length) == 1
' "$EVIDENCE_DIR/release-09d-evidence.json" >/dev/null
echo "release-proof 09D ${EXPECTED_RELEASE_STATUS}: run_id=$RUN_ID final_tasks=$FINAL_TOTAL initial_brotli_bytes=$INITIAL_BROTLI_BYTES evidence=$EVIDENCE_DIR/release-09d-evidence.json"
