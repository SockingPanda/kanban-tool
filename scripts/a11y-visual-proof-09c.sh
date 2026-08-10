#!/usr/bin/env bash
set -euo pipefail
umask 077
export LC_ALL=C

ROOT="$(cd "$(dirname "$0")/.." && pwd -P)"
PORT="${KANBAN_A11Y_PORT:-18723}"
BASE_URL="http://127.0.0.1:$PORT"
RUN_ID="${KANBAN_A11Y_RUN_ID:-09c-$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n')}"
EVIDENCE_DIR_INPUT="${KANBAN_A11Y_EVIDENCE_DIR:-$ROOT/output/release/a11y-09c}"

for tool in awk chmod cp curl find git grep jq just kill ln mkdir mktemp mv od pnpm readlink realpath rmdir sed seq sha256sum sleep sort ss stat setsid tr; do
  command -v "$tool" >/dev/null 2>&1 || { echo "error: missing required tool: $tool" >&2; exit 1; }
done

MODE="${KANBAN_A11Y_MODE:-formal}"
case "$MODE" in
  formal|diagnostic) ;;
  *) echo "error: KANBAN_A11Y_MODE must be formal or diagnostic" >&2; exit 1 ;;
esac
START_SHA="$(git -C "$ROOT" rev-parse HEAD)"
START_CLEAN_STATE="clean"
[[ -z "$(git -C "$ROOT" status --porcelain --untracked-files=all)" ]] || START_CLEAN_STATE="dirty"
if [[ "$MODE" == "formal" && "$START_CLEAN_STATE" != "clean" ]]; then
  echo "error: formal 09C proof requires a clean checkout; use KANBAN_A11Y_MODE=diagnostic for dirty targeted runs" >&2
  exit 1
fi

case "$EVIDENCE_DIR_INPUT" in
  /*) EVIDENCE_DIR="$EVIDENCE_DIR_INPUT" ;;
  *) EVIDENCE_DIR="$ROOT/$EVIDENCE_DIR_INPUT" ;;
esac
case "$EVIDENCE_DIR" in
  *"/../"*|*"/.."|../*|..|*"/./"*|./*)
    echo "error: evidence dir must not contain traversal or dot components" >&2
    exit 1
    ;;
esac
EVIDENCE_LEXICAL="$(realpath -m -s -- "$EVIDENCE_DIR")"
EVIDENCE_PHYSICAL="$(realpath -m -- "$EVIDENCE_DIR")"
[[ "$EVIDENCE_LEXICAL" == "$EVIDENCE_PHYSICAL" ]] || {
  echo "error: evidence dir parent must not contain symlinks: $EVIDENCE_DIR" >&2
  exit 1
}
mkdir -p -m 700 -- "$EVIDENCE_DIR"
EVIDENCE_DIR="$(realpath -e -- "$EVIDENCE_DIR")"
[[ "$EVIDENCE_DIR" == "$EVIDENCE_LEXICAL" ]] || {
  echo "error: evidence dir canonical path differs from lexical path: $EVIDENCE_DIR" >&2
  exit 1
}
[[ ! -L "$EVIDENCE_DIR" && "$(stat -c '%u:%a' -- "$EVIDENCE_DIR")" == "$(id -u):700" ]] || {
  echo "error: evidence dir must be uid-owned mode 700: $EVIDENCE_DIR" >&2
  exit 1
}

STALE_TEMP="$(find "$EVIDENCE_DIR" -mindepth 1 -maxdepth 1 -name '*.tmp' -print -quit)"
[[ -z "$STALE_TEMP" ]] || {
  echo "error: stale evidence temp exists; remove before rerun: $STALE_TEMP" >&2
  exit 1
}

for destination in \
  "$EVIDENCE_DIR/keyboard-chromium.json" \
  "$EVIDENCE_DIR/keyboard-firefox.json" \
  "$EVIDENCE_DIR/visual-chromium.json" \
  "$EVIDENCE_DIR/host.log"; do
  [[ ! -L "$destination" ]] || {
    echo "error: evidence destination is symlink: $destination" >&2
    exit 1
  }
  if [[ -e "$destination" ]]; then
    [[ -f "$destination" ]] || {
      echo "error: evidence destination is not a regular file: $destination" >&2
      exit 1
    }
    find "$destination" -maxdepth 0 -type f -delete
  fi
done

TMP_ROOT="$(mktemp -d /tmp/kanban-a11y-09c.XXXXXX)"
chmod 700 -- "$TMP_ROOT"
DB_PATH="$TMP_ROOT/canonical.db"
HOST_LOG="$TMP_ROOT/host.log"
HOST_PID=""
HOST_START_TIME=""
HOST_OWNER_PID=""
HOST_OWNER_START_TIME=""
HOST_STOPPED=0
HOST_LOG_PUBLISHED=0
TMP_REMOVED=0

proc_start_time() {
  local pid="$1"
  [[ "$pid" =~ ^[0-9]+$ ]] || return 1
  [[ -r "/proc/$pid/stat" ]] || return 1
  awk '{print $22}' "/proc/$pid/stat"
}

listener_owner_pids() {
  ss -H -ltnp "sport = :$PORT" 2>/dev/null \
    | grep -o 'pid=[0-9]*' \
    | sed 's/^pid=//' \
    | sort -u || true
}

listener_present() {
  ss -H -ltn "sport = :$PORT" 2>/dev/null | grep -q .
}

assert_listener_owner() {
  local -a owners=()
  mapfile -t owners < <(listener_owner_pids)
  if (( ${#owners[@]} != 1 )) || [[ "${owners[0]:-}" != "$HOST_PID" ]]; then
    printf 'error: port %s listener owner is not kanban pid=%s (owners=%s)\n' "$PORT" "$HOST_PID" "${owners[*]:-none}" >&2
    return 1
  fi
  HOST_OWNER_PID="${owners[0]}"
  local current_start="$(proc_start_time "$HOST_PID")"
  HOST_OWNER_START_TIME="$(proc_start_time "$HOST_OWNER_PID")"
  [[ -n "$current_start" && "$current_start" == "$HOST_START_TIME" && "$HOST_START_TIME" == "$HOST_OWNER_START_TIME" ]] || {
    echo "error: listener owner starttime does not match kanban process" >&2
    return 1
  }
  printf '[launcher] host_pid=%s host_start_time=%s listener_owner_pid=%s listener_owner_start_time=%s\n' \
    "$HOST_PID" "$HOST_START_TIME" "$HOST_OWNER_PID" "$HOST_OWNER_START_TIME" >>"$HOST_LOG"
}

verify_typed_health() {
  local health
  health="$(curl --fail --silent --show-error --max-time 2 "$BASE_URL/health")" || return 1
  jq -e --arg db_path "$DB_PATH" '.data.ok == true and .data.db == "turso" and .data.db_path == $db_path' <<<"$health" >/dev/null || {
    echo "error: typed health did not bind turso/$DB_PATH" >&2
    printf '%s\n' "$health" >&2
    return 1
  }
}

verify_host_identity() {
  HOST_EXE="$(readlink -e "/proc/$HOST_PID/exe")"
  KANBAN_SHA256="$(sha256sum "$KANBAN" | awk '{print $1}')"
  local -a HOST_ARGV_TOKENS=()
  mapfile -d '' -t HOST_ARGV_TOKENS <"/proc/$HOST_PID/cmdline"
  HOST_ARGV="$(printf '%s ' "${HOST_ARGV_TOKENS[@]}")"
  HOST_ARGV="${HOST_ARGV% }"
  argv_has_token() {
    local expected="$1"
    local token
    for token in "${HOST_ARGV_TOKENS[@]}"; do
      [[ "$token" == "$expected" ]] && return 0
    done
    return 1
  }
  argv_has_pair() {
    local key="$1"
    local value="$2"
    local index
    for (( index = 0; index + 1 < ${#HOST_ARGV_TOKENS[@]}; index += 1 )); do
      if [[ "${HOST_ARGV_TOKENS[index]}" == "$key" && "${HOST_ARGV_TOKENS[index + 1]}" == "$value" ]]; then
        return 0
      fi
    done
    return 1
  }
  [[ "$HOST_EXE" == "$(realpath -e -- "$KANBAN")" ]] || {
    echo "error: host /proc/exe does not match kanban binary" >&2
    return 1
  }
  argv_has_token serve \
    && argv_has_pair --db "$DB_PATH" \
    && argv_has_pair --host 127.0.0.1 \
    && argv_has_pair --port "$PORT" \
    && argv_has_pair --web-dir "$ROOT/apps/web/dist" || {
    echo "error: host argv does not bind serve/port/db/web-dir: $HOST_ARGV" >&2
    return 1
  }
  printf '[launcher] host_exe=%s kanban_sha256=%s host_argv=%s port=%s db_path=%s web_dir=%s\n' \
    "$HOST_EXE" "$KANBAN_SHA256" "$HOST_ARGV" "$PORT" "$DB_PATH" "$ROOT/apps/web/dist" >>"$HOST_LOG"
}

wait_health() {
  local attempts=0
  while (( attempts < 120 )); do
    if curl --fail --silent --show-error --max-time 2 "$BASE_URL/health" >/dev/null 2>&1; then return 0; fi
    if [[ -n "$HOST_PID" ]] && ! kill -0 "$HOST_PID" 2>/dev/null; then
      echo "error: kanban serve exited before health became ready" >&2
      sed -n '1,240p' "$HOST_LOG" >&2 || true
      return 1
    fi
    attempts=$((attempts + 1))
    sleep 0.25
  done
  echo "error: kanban serve health did not become ready: $BASE_URL" >&2
  sed -n '1,240p' "$HOST_LOG" >&2 || true
  return 1
}

stop_host() {
  (( HOST_STOPPED == 1 )) && return 0
  local status=0
  if [[ -n "$HOST_PID" ]] && kill -0 "$HOST_PID" 2>/dev/null; then
    local current_start="$(proc_start_time "$HOST_PID" 2>/dev/null || true)"
    if [[ -z "$HOST_START_TIME" || "$current_start" != "$HOST_START_TIME" ]]; then
      echo "error: kanban pid/starttime changed before cleanup" >&2
      status=1
    else
      local -a owners=()
      mapfile -t owners < <(listener_owner_pids)
      if (( ${#owners[@]} != 1 )) || [[ "${owners[0]:-}" != "$HOST_PID" ]]; then
        printf 'error: listener owner changed before cleanup: expected pid=%s got=%s\n' "$HOST_PID" "${owners[*]:-none}" >&2
        status=1
      fi
      kill -INT -- "-$HOST_PID" 2>/dev/null || true
      for _ in $(seq 1 80); do
        kill -0 "$HOST_PID" 2>/dev/null || break
        sleep 0.1
      done
      kill -TERM -- "-$HOST_PID" 2>/dev/null || true
      kill -KILL -- "-$HOST_PID" 2>/dev/null || true
      wait "$HOST_PID" 2>/dev/null || true
    fi
  fi
  if [[ -n "$HOST_PID" ]] && kill -0 "$HOST_PID" 2>/dev/null; then
    echo "error: kanban pid remains after cleanup: $HOST_PID" >&2
    status=1
  fi
  if listener_present; then
    echo "error: port $PORT remains bound after cleanup" >&2
    status=1
  fi
  HOST_STOPPED=1
  return "$status"
}

publish_host_log() {
  (( HOST_LOG_PUBLISHED == 1 )) && return 0
  [[ -f "$HOST_LOG" && ! -L "$HOST_LOG" ]] || {
    echo "error: host log missing or symlink: $HOST_LOG" >&2
    return 1
  }
  local destination="$EVIDENCE_DIR/host.log"
  local temporary="$EVIDENCE_DIR/.host.log.$$.tmp"
  [[ ! -e "$destination" && ! -L "$destination" ]] || {
    echo "error: host log destination already exists: $destination" >&2
    return 1
  }
  [[ ! -e "$temporary" && ! -L "$temporary" ]] || {
    echo "error: host log temporary destination already exists: $temporary" >&2
    return 1
  }
  cp -- "$HOST_LOG" "$temporary" || return 1
  chmod 600 -- "$temporary" || return 1
  if ! mv --no-clobber -T -- "$temporary" "$destination"; then
    find "$temporary" -maxdepth 0 -type f -delete 2>/dev/null || true
    return 1
  fi
  [[ ! -e "$temporary" && ! -L "$temporary" ]] || {
    find "$temporary" -maxdepth 0 -type f -delete 2>/dev/null || true
    return 1
  }
  [[ -f "$destination" && ! -L "$destination" && "$(stat -c '%a:%h' -- "$destination")" == "600:1" ]] || return 1
  HOST_LOG_PUBLISHED=1
}

update_browser_end_metadata() {
  local end_sha="$1"
  local end_clean="$2"
  local status=0
  for evidence in keyboard-chromium.json keyboard-firefox.json visual-chromium.json; do
    local path="$EVIDENCE_DIR/$evidence"
    local temporary="$path.$$.tmp"
    if [[ ! -f "$path" || -L "$path" || "$(stat -c '%a:%h' -- "$path")" != "600:1" ]]; then
      echo "error: cannot update end metadata for non-private evidence: $path" >&2
      status=1
      continue
    fi
    if [[ -e "$temporary" || -L "$temporary" ]]; then
      echo "error: browser evidence temporary destination already exists: $temporary" >&2
      status=1
      continue
    fi
    if ! jq --arg end_sha "$end_sha" --arg end_clean "$end_clean" \
      '.host.end_sha = $end_sha | .host.end_clean = $end_clean' "$path" >"$temporary"; then
      echo "error: failed to update end metadata: $path" >&2
      find "$temporary" -maxdepth 0 -type f -delete 2>/dev/null || true
      status=1
      continue
    fi
    chmod 600 -- "$temporary" || status=1
    if [[ "$status" -ne 0 || ! -f "$temporary" || -L "$temporary" || "$(stat -c '%a:%h' -- "$temporary")" != "600:1" ]]; then
      echo "error: browser evidence temporary is not private: $temporary" >&2
      find "$temporary" -maxdepth 0 -type f -delete 2>/dev/null || true
      status=1
      continue
    fi
    if ! mv -T -- "$temporary" "$path"; then
      echo "error: failed to atomically update end metadata: $path" >&2
      find "$temporary" -maxdepth 0 -type f -delete 2>/dev/null || true
      status=1
      continue
    fi
    [[ -f "$path" && ! -L "$path" && "$(stat -c '%a:%h' -- "$path")" == "600:1" ]] || status=1
  done
  return "$status"
}

remove_tmp_root() {
  (( TMP_REMOVED == 1 )) && return 0
  local status=0
  if [[ "$TMP_ROOT" == /tmp/kanban-a11y-09c.* && -d "$TMP_ROOT" && ! -L "$TMP_ROOT" ]]; then
    find -- "$TMP_ROOT" -mindepth 1 -depth -delete || status=1
    rmdir -- "$TMP_ROOT" || status=1
  elif [[ -e "$TMP_ROOT" || -L "$TMP_ROOT" ]]; then
    echo "error: unexpected temporary root path: $TMP_ROOT" >&2
    status=1
  fi
  [[ ! -e "$TMP_ROOT" && ! -L "$TMP_ROOT" ]] || status=1
  TMP_REMOVED=1
  return "$status"
}

cleanup() {
  local status=$?
  set +e
  stop_host || status=1
  publish_host_log || status=1
  remove_tmp_root || status=1
  exit "$status"
}
trap cleanup EXIT

if listener_present; then
  echo "error: a listener already owns $BASE_URL" >&2
  exit 1
fi

if curl --silent --max-time 0.5 "$BASE_URL/health" >/dev/null 2>&1; then
  echo "error: a host already serves $BASE_URL" >&2
  exit 1
fi

if listener_present; then
  echo "error: port $PORT became bound before launch" >&2
  exit 1
fi

just --justfile "$ROOT/justfile" web-build
"$ROOT/scripts/cargo-build-lock.sh" -- cargo build --locked -p kanban-cli
TARGET_DIR="$("$ROOT/scripts/cargo-build-lock.sh" --print-target-dir)"
KANBAN="$TARGET_DIR/debug/kanban"
[[ -x "$KANBAN" && ! -L "$KANBAN" ]] || { echo "error: kanban binary missing: $KANBAN" >&2; exit 1; }

if listener_present; then
  echo "error: port $PORT became bound during build" >&2
  exit 1
fi

setsid env KANBAN_ACTOR=a11y-09c "$KANBAN" --db "$DB_PATH" serve --host 127.0.0.1 --port "$PORT" --web-dir "$ROOT/apps/web/dist" >"$HOST_LOG" 2>&1 &
HOST_PID="$!"
HOST_START_TIME="$(proc_start_time "$HOST_PID" 2>/dev/null || true)"
[[ -n "$HOST_START_TIME" ]] || { echo "error: could not bind kanban pid starttime" >&2; exit 1; }
wait_health
assert_listener_owner
verify_typed_health
verify_host_identity

BOARDS_JSON="$(KANBAN_SERVER_URL="$BASE_URL" "$KANBAN" --json --board default board list)"
if ! jq -e '.data | any(.slug == "default")' <<<"$BOARDS_JSON" >/dev/null; then
  KANBAN_SERVER_URL="$BASE_URL" "$KANBAN" --json --board default board create default --name "A11y 09C" >/dev/null
fi
KANBAN_SERVER_URL="$BASE_URL" "$KANBAN" --json --board default task create "A11y Seed Task" --status todo --task-id t_a11y_seed >/dev/null

export KANBAN_A11Y_BASE_URL="$BASE_URL"
export KANBAN_A11Y_RUN_ID="$RUN_ID"
export KANBAN_A11Y_EVIDENCE_DIR="$EVIDENCE_DIR"
export KANBAN_A11Y_MODE="$MODE"
export KANBAN_A11Y_START_SHA="$START_SHA"
export KANBAN_A11Y_END_SHA="$START_SHA"
export KANBAN_A11Y_START_CLEAN="$START_CLEAN_STATE"
export KANBAN_A11Y_END_CLEAN="$START_CLEAN_STATE"
export KANBAN_A11Y_PORT="$PORT"
export KANBAN_A11Y_HOST_PID="$HOST_PID"
export KANBAN_A11Y_HOST_START_TIME="$HOST_START_TIME"
export KANBAN_A11Y_LISTENER_OWNER_PID="$HOST_OWNER_PID"
export KANBAN_A11Y_LISTENER_OWNER_START_TIME="$HOST_OWNER_START_TIME"
export KANBAN_A11Y_HOST_EXE="$HOST_EXE"
export KANBAN_A11Y_KANBAN_SHA256="$KANBAN_SHA256"
export KANBAN_A11Y_HOST_ARGV="$HOST_ARGV"
export KANBAN_A11Y_WEB_DIR="$ROOT/apps/web/dist"
export KANBAN_A11Y_DB_PATH="$DB_PATH"

TEST_STATUS=0
set +e
pnpm --filter '@kanban-tool/web' exec playwright test --config playwright.a11y.config.ts
KEYBOARD_STATUS=$?
if [[ "${KANBAN_A11Y_UPDATE_SNAPSHOTS:-0}" == "1" ]]; then
  pnpm --filter '@kanban-tool/web' exec playwright test --config playwright.visual.config.ts --update-snapshots
  VISUAL_UPDATE_STATUS=$?
else
  VISUAL_UPDATE_STATUS=0
fi
pnpm --filter '@kanban-tool/web' exec playwright test --config playwright.visual.config.ts
VISUAL_STATUS=$?
set -e
(( KEYBOARD_STATUS == 0 )) || TEST_STATUS=1
(( VISUAL_UPDATE_STATUS == 0 )) || TEST_STATUS=1
(( VISUAL_STATUS == 0 )) || TEST_STATUS=1

END_SHA="$(git -C "$ROOT" rev-parse HEAD)"
END_CLEAN_STATE="clean"
[[ -z "$(git -C "$ROOT" status --porcelain --untracked-files=all)" ]] || END_CLEAN_STATE="dirty"
printf '[launcher] mode=%s start_sha=%s start_clean=%s end_sha=%s end_clean=%s\n' \
  "$MODE" "$START_SHA" "$START_CLEAN_STATE" "$END_SHA" "$END_CLEAN_STATE" >>"$HOST_LOG"
FINAL_STATUS=0
(( TEST_STATUS == 0 )) || FINAL_STATUS=1
if [[ "$END_SHA" != "$START_SHA" || "$END_CLEAN_STATE" != "$START_CLEAN_STATE" ]]; then
  echo "error: checkout SHA/clean state changed during 09C proof" >&2
  FINAL_STATUS=1
fi
if [[ "$MODE" == "formal" && ( "$END_SHA" != "$START_SHA" || "$END_CLEAN_STATE" != "clean" ) ]]; then
  echo "error: formal 09C proof requires unchanged clean SHA" >&2
  FINAL_STATUS=1
fi
update_browser_end_metadata "$END_SHA" "$END_CLEAN_STATE" || FINAL_STATUS=1
stop_host || FINAL_STATUS=1
publish_host_log || FINAL_STATUS=1
remove_tmp_root || FINAL_STATUS=1
if (( FINAL_STATUS != 0 )); then
  exit "$FINAL_STATUS"
fi
trap - EXIT

EXPECTED_FILES="host.log keyboard-chromium.json keyboard-firefox.json visual-chromium.json"
ACTUAL_FILES="$(find "$EVIDENCE_DIR" -mindepth 1 -maxdepth 1 -printf '%f\n' | sort | tr '\n' ' ' | sed 's/ *$//')"
[[ "$ACTUAL_FILES" == "$EXPECTED_FILES" ]] || {
  echo "error: evidence directory must contain exactly host.log and three JSON files; got: $ACTUAL_FILES" >&2
  exit 1
}
STALE_TEMP="$(find "$EVIDENCE_DIR" -mindepth 1 -maxdepth 1 -name '*.tmp' -print -quit)"
[[ -z "$STALE_TEMP" ]] || {
  echo "error: evidence temp remains after proof: $STALE_TEMP" >&2
  exit 1
}
for evidence in keyboard-chromium.json keyboard-firefox.json visual-chromium.json; do
  path="$EVIDENCE_DIR/$evidence"
  [[ -f "$path" && ! -L "$path" ]] || { echo "error: evidence missing: $path" >&2; exit 1; }
  jq -e --arg run_id "$RUN_ID" '.run_id == $run_id' "$path" >/dev/null || {
    echo "error: evidence run_id mismatch: $path" >&2
    exit 1
  }
done
[[ -f "$EVIDENCE_DIR/host.log" && ! -L "$EVIDENCE_DIR/host.log" ]] || { echo "error: host.log missing" >&2; exit 1; }
if [[ "$MODE" == "formal" ]]; then
  KEYBOARD_FILTER='(
    .axe_gate == "passed"
    and .keyboard_gate == "passed"
    and .incomplete_disposition_gate == "passed"
    and .incomplete_evidence_gate == "passed"
    and ([.axe_runs[].label] | sort) == (.expected_axe_labels | sort)
    and ([.keyboard[].label] | sort) == (.expected_keyboard_labels | sort)
    and all(.axe_runs[];
      (.violations | length) == 0
      and all(.incomplete[];
        .disposition != null
        and .disposition.reviewed == true
        and (.disposition.rationale | length) > 0
        and all(.nodes[]; .computed != null and .computed.error == null and .computed.accessible_name != null and .computed.font_size != null and .computed.font_weight != null and .computed.opacity == "1" and .computed.contrast_ratio != null and .computed.contrast_ratio >= 4.5)
      )
      and .context.document_language == "zh-CN"
      and .context.navigator_language == "zh-CN"
      and ((.context.intl_locale | ascii_downcase) == "zh-cn" or (.context.intl_locale | ascii_downcase) == "zh-hans-cn")
      and .context.color_scheme_light == true
      and .context.color_scheme_dark == false
      and .context.reduced_motion_reduce == true
      and .context.theme_state == "system"
      and .context.astryx_theme == "neutral"
    )
    and all(.keyboard[]; .focusVisible == true)
    and (.page_errors | length) == 0
    and .host.mode == $mode
    and .host.start_sha == $start_sha
    and .host.end_sha == $end_sha
    and .host.start_clean == "clean"
    and .host.end_clean == $end_clean
    and .host.pid == $host_pid
    and .host.start_time == $host_start_time
    and .host.listener_owner_pid == $host_owner_pid
    and .host.listener_owner_start_time == $host_owner_start_time
    and .host.exe == $host_exe
    and .host.sha256 == $kanban_sha256
    and .host.argv == $host_argv
    and .host.port == $port
    and .host.db == "turso"
    and .host.db_path == $db_path
    and .host.web_dir == $web_dir
  )'
  VISUAL_FILTER='(
    .visual_gate == "passed"
    and ([.screenshots[].name] | sort) == (.expected_screenshot_names | sort)
    and all(.screenshots[];
      .context.document_language == "zh-CN"
      and .context.navigator_language == "zh-CN"
      and ((.context.intl_locale | ascii_downcase) == "zh-cn" or (.context.intl_locale | ascii_downcase) == "zh-hans-cn")
      and .context.color_scheme_light == true
      and .context.color_scheme_dark == false
      and .context.reduced_motion_reduce == true
      and .context.theme_state == "system"
      and .context.astryx_theme == "neutral"
    )
    and (.failures | length) == 0
    and (.page_errors | length) == 0
    and .host.mode == $mode
    and .host.start_sha == $start_sha
    and .host.end_sha == $end_sha
    and .host.start_clean == "clean"
    and .host.end_clean == $end_clean
    and .host.pid == $host_pid
    and .host.start_time == $host_start_time
    and .host.listener_owner_pid == $host_owner_pid
    and .host.listener_owner_start_time == $host_owner_start_time
    and .host.exe == $host_exe
    and .host.sha256 == $kanban_sha256
    and .host.argv == $host_argv
    and .host.port == $port
    and .host.db == "turso"
    and .host.db_path == $db_path
    and .host.web_dir == $web_dir
  )'
  for browser in chromium firefox; do
    jq -e --arg mode "$MODE" --arg start_sha "$START_SHA" --arg end_sha "$END_SHA" --arg end_clean "$END_CLEAN_STATE" --arg host_pid "$HOST_PID" \
      --arg host_start_time "$HOST_START_TIME" --arg host_owner_pid "$HOST_OWNER_PID" \
      --arg host_owner_start_time "$HOST_OWNER_START_TIME" --arg host_exe "$HOST_EXE" \
      --arg kanban_sha256 "$KANBAN_SHA256" --arg host_argv "$HOST_ARGV" --arg port "$PORT" \
      --arg db_path "$DB_PATH" --arg web_dir "$ROOT/apps/web/dist" "$KEYBOARD_FILTER" \
      "$EVIDENCE_DIR/keyboard-$browser.json" >/dev/null || {
      echo "error: keyboard evidence aggregate validation failed: $browser" >&2
      exit 1
    }
  done
  jq -e --arg mode "$MODE" --arg start_sha "$START_SHA" --arg end_sha "$END_SHA" --arg end_clean "$END_CLEAN_STATE" --arg host_pid "$HOST_PID" \
    --arg host_start_time "$HOST_START_TIME" --arg host_owner_pid "$HOST_OWNER_PID" \
    --arg host_owner_start_time "$HOST_OWNER_START_TIME" --arg host_exe "$HOST_EXE" \
    --arg kanban_sha256 "$KANBAN_SHA256" --arg host_argv "$HOST_ARGV" --arg port "$PORT" \
    --arg db_path "$DB_PATH" --arg web_dir "$ROOT/apps/web/dist" "$VISUAL_FILTER" \
    "$EVIDENCE_DIR/visual-chromium.json" >/dev/null || {
    echo "error: visual evidence aggregate validation failed" >&2
    exit 1
  }
else
  echo "09C diagnostic run: aggregate gates intentionally not promoted to formal pass" >&2
fi
if [[ "$MODE" == "formal" ]]; then
  echo "09C axe/keyboard/visual proof passed: run_id=$RUN_ID evidence=$EVIDENCE_DIR"
else
  echo "09C diagnostic evidence complete: run_id=$RUN_ID evidence=$EVIDENCE_DIR"
fi
