#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
PROOF_MODE=0
if [[ "${1:-}" == "--proof" ]]; then
  PROOF_MODE=1
  shift
fi
SKIP_RECEIPT="${KANBAN_DESKTOP_PACKAGE_PROOF_SKIP_RECEIPT:-0}"
if (( PROOF_MODE )); then
  umask 077
fi

DEB_DIR="$("$LOCK" --print-target-dir)/release/bundle/deb"
if (( PROOF_MODE )); then
  TMP_ROOT="$(mktemp -d "/tmp/kanban-desktop-smoke.XXXXXX")"
else
  TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/kanban-desktop-smoke.XXXXXX")"
fi
EXTRACTED="$TMP_ROOT/extracted"
RUNTIME_HOME="$TMP_ROOT/home"
RUNTIME_XDG="$TMP_ROOT/xdg-runtime"
WRAPPER_PID=""
APP_BINARY=""
SIDECAR_PATH=""
ROLLBACK_WRAPPER_PID=""
ROLLBACK_APP_BINARY=""
ROLLBACK_SIDECAR_PATH=""
ROLLBACK_SNAPSHOT_BEFORE=""
ROLLBACK_SNAPSHOT_AFTER=""
AUX_PID=""
AUX_LAUNCH_PID=""
AUX_GROUP=""
AUX_START=""
AUX_PORT=18722
AUX_URL="http://127.0.0.1:$AUX_PORT"
START_SHA=""
START_CLEAN=0
RUN_ID=""
FORMAL_EVIDENCE_PATH="$ROOT/output/release/desktop-package-evidence.json"
DIAGNOSTIC_EVIDENCE_PATH="$ROOT/output/release/desktop-package-diagnostic-evidence.json"
RECEIPT_PATH="$ROOT/output/release/desktop-package-receipt.json"
if [[ "$SKIP_RECEIPT" == "1" ]]; then
  EVIDENCE_PATH="$DIAGNOSTIC_EVIDENCE_PATH"
else
  EVIDENCE_PATH="$FORMAL_EVIDENCE_PATH"
fi
EVIDENCE_DIR="$ROOT/output/release"

if (( PROOF_MODE )); then
  START_SHA="$(git -C "$ROOT" rev-parse HEAD)"
  START_CLEAN=1
  [[ -z "$(git -C "$ROOT" status --porcelain=v1)" ]] || START_CLEAN=0
  RUN_ID="09e-$(printf '%s' "$$-$(date +%s%N)" | sha256sum | cut -c1-32)"
fi

executable_pids() {
  local expected="$1"
  local proc_path pid executable
  [[ -n "$expected" ]] || return 0
  for proc_path in /proc/[0-9]*; do
    [[ -d "$proc_path" ]] || continue
    pid="${proc_path##*/}"
    executable="$(readlink -f "$proc_path/exe" 2>/dev/null || true)"
    [[ "$executable" == "$expected" ]] && printf '%s\n' "$pid"
  done
}

app_pid_for_path() {
  executable_pids "$APP_BINARY" | head -n 1
}

sidecar_pids() {
  executable_pids "$SIDECAR_PATH"
}

listener_pids() {
  local port="$1"
  ss -H -ltnp "sport = :$port" 2>/dev/null \
    | grep -o 'pid=[0-9]*' \
    | cut -d= -f2 \
    | sort -nu || true
}

listener_any() {
  local port="$1"
  ss -H -ltn "sport = :$port" 2>/dev/null || true
}

assert_aux_owner() {
  local -a owners=()
  mapfile -t owners < <(listener_pids "$AUX_PORT")
  [[ "${#owners[@]}" -eq 1 && "${owners[0]}" == "$AUX_PID" ]] || return 1
  kill -0 "$AUX_PID" 2>/dev/null || return 1
  [[ "$(aux_process_start "$AUX_PID" || true)" == "$AUX_START" ]] || return 1
  [[ "$(readlink -f "/proc/$AUX_PID/exe" 2>/dev/null || true)" == "$(readlink -f "$SIDECAR_PATH")" ]]
}

prepare_evidence_destination() {
  [[ ! -L "$ROOT/output" && ! -L "$EVIDENCE_DIR" ]] || {
    echo "error: Desktop package evidence directory must not contain symlink components" >&2
    exit 1
  }
  mkdir -p "$EVIDENCE_DIR"
  [[ ! -L "$ROOT/output" && ! -L "$EVIDENCE_DIR" ]] || {
    echo "error: Desktop package evidence directory became a symlink" >&2
    exit 1
  }
  chmod 700 "$EVIDENCE_DIR"
  [[ "$(stat -c '%a' "$EVIDENCE_DIR")" == "700" ]] || {
    echo "error: Desktop package evidence directory must be private mode=700" >&2
    exit 1
  }
  [[ "$(stat -c '%u' "$EVIDENCE_DIR")" == "$(stat -c '%u' "$ROOT")" ]] || {
    echo "error: Desktop package evidence directory owner must match workspace owner" >&2
    exit 1
  }
}

rollback_app_pids() {
  executable_pids "$ROLLBACK_APP_BINARY"
}

rollback_sidecar_pids() {
  executable_pids "$ROLLBACK_SIDECAR_PATH"
}

aux_process_start() {
  awk '{print $22}' "/proc/$1/stat" 2>/dev/null
}

start_aux_host() {
  local db_path="$1" web_dir="$2" log_path="$3"
  [[ -z "$(listener_any "$AUX_PORT")" ]] || return 1
  setsid env KANBAN_ACTOR=desktop-package-09e "$SIDECAR_PATH" --db "$db_path" \
    serve --host 127.0.0.1 --port "$AUX_PORT" --web-dir "$web_dir" >"$log_path" 2>&1 &
  AUX_LAUNCH_PID=$!
  AUX_GROUP="$AUX_LAUNCH_PID"
  for _ in {1..100}; do
    mapfile -t aux_owners < <(listener_pids "$AUX_PORT")
    if [[ "${#aux_owners[@]}" -eq 1 ]]; then
      candidate="${aux_owners[0]}"
      candidate_exe="$(readlink -f "/proc/$candidate/exe" 2>/dev/null || true)"
      expected_exe="$(readlink -f "$SIDECAR_PATH")"
      if [[ "$candidate_exe" == "$expected_exe" && -r "/proc/$candidate/stat" ]]; then
        AUX_PID="$candidate"
        AUX_START="$(aux_process_start "$AUX_PID")"
        if [[ -n "$AUX_START" ]] \
          && assert_aux_owner \
          && curl --silent --show-error --fail --max-time 0.5 "$AUX_URL/health" >/dev/null 2>&1; then
          return 0
        fi
      fi
    fi
    if ! kill -0 "$AUX_LAUNCH_PID" 2>/dev/null && [[ -z "$(listener_any "$AUX_PORT")" ]]; then return 1; fi
    sleep 0.1
  done
  return 1
}

stop_aux_host() {
  [[ -n "$AUX_PID" || -n "$AUX_LAUNCH_PID" ]] || return 0
  if kill -0 "$AUX_PID" 2>/dev/null && [[ "$(aux_process_start "$AUX_PID" || true)" == "$AUX_START" ]]; then
    kill -INT -- "-${AUX_GROUP:-$AUX_PID}" 2>/dev/null || kill -TERM "$AUX_PID" 2>/dev/null || true
    for _ in {1..100}; do
      if ! kill -0 "$AUX_PID" 2>/dev/null || [[ "$(awk '{print $3}' "/proc/$AUX_PID/stat" 2>/dev/null || true)" == "Z" ]]; then break; fi
      sleep 0.1
    done
    if kill -0 "$AUX_PID" 2>/dev/null; then
      kill -KILL -- "-${AUX_GROUP:-$AUX_PID}" 2>/dev/null || kill -KILL "$AUX_PID" 2>/dev/null || true
    fi
  fi
  if [[ -n "$AUX_LAUNCH_PID" ]]; then wait "$AUX_LAUNCH_PID" 2>/dev/null || true; fi
  if [[ -n "$AUX_PID" && "$AUX_PID" != "$AUX_LAUNCH_PID" ]]; then wait "$AUX_PID" 2>/dev/null || true; fi
  AUX_PID=""
  AUX_LAUNCH_PID=""
  AUX_GROUP=""
  AUX_START=""
}

terminate_exact() {
  local expected="$1" signal="$2" pid
  [[ -n "$expected" ]] || return 0
  while read -r pid; do
    [[ -n "$pid" ]] && kill -s "$signal" "$pid" 2>/dev/null || true
  done < <(executable_pids "$expected")
}

cleanup_runtime() {
  set +e
  terminate_exact "${APP_BINARY:-}" TERM
  terminate_exact "${SIDECAR_PATH:-}" TERM
  terminate_exact "${ROLLBACK_APP_BINARY:-}" TERM
  terminate_exact "${ROLLBACK_SIDECAR_PATH:-}" TERM
  if [[ -n "${WRAPPER_PID:-}" ]] && kill -0 "$WRAPPER_PID" 2>/dev/null; then
    kill -TERM "$WRAPPER_PID" 2>/dev/null || true
  fi
  if [[ -n "${ROLLBACK_WRAPPER_PID:-}" ]] && kill -0 "$ROLLBACK_WRAPPER_PID" 2>/dev/null; then
    kill -TERM "$ROLLBACK_WRAPPER_PID" 2>/dev/null || true
  fi
  for _ in {1..80}; do
    local app_left sidecar_left rollback_app_left rollback_sidecar_left
    app_left="$(executable_pids "${APP_BINARY:-}" || true)"
    sidecar_left="$(executable_pids "${SIDECAR_PATH:-}" || true)"
    rollback_app_left="$(executable_pids "${ROLLBACK_APP_BINARY:-}" || true)"
    rollback_sidecar_left="$(executable_pids "${ROLLBACK_SIDECAR_PATH:-}" || true)"
    [[ -z "$app_left" && -z "$sidecar_left" && -z "$rollback_app_left" && -z "$rollback_sidecar_left" ]] && break
    sleep 0.1
  done
  terminate_exact "${APP_BINARY:-}" KILL
  terminate_exact "${SIDECAR_PATH:-}" KILL
  terminate_exact "${ROLLBACK_APP_BINARY:-}" KILL
  terminate_exact "${ROLLBACK_SIDECAR_PATH:-}" KILL
  if [[ -n "${AUX_LAUNCH_PID:-}" || -n "${AUX_PID:-}" ]]; then
    kill -INT -- "-${AUX_GROUP:-$AUX_PID}" 2>/dev/null || kill -TERM "${AUX_PID:-$AUX_LAUNCH_PID}" 2>/dev/null || true
    kill -KILL -- "-${AUX_GROUP:-$AUX_PID}" 2>/dev/null || true
  fi
  if [[ -n "${WRAPPER_PID:-}" ]]; then wait "$WRAPPER_PID" 2>/dev/null || true; fi
  if [[ -n "${ROLLBACK_WRAPPER_PID:-}" ]]; then wait "$ROLLBACK_WRAPPER_PID" 2>/dev/null || true; fi
  WRAPPER_PID=""
  ROLLBACK_WRAPPER_PID=""
  if [[ -n "${AUX_LAUNCH_PID:-}" ]]; then wait "$AUX_LAUNCH_PID" 2>/dev/null || true; fi
  if [[ -n "${AUX_PID:-}" && "$AUX_PID" != "${AUX_LAUNCH_PID:-}" ]]; then wait "$AUX_PID" 2>/dev/null || true; fi
  AUX_PID=""
  AUX_LAUNCH_PID=""
  AUX_GROUP=""
  AUX_START=""
  set -e
}

cleanup() {
  cleanup_runtime
  rm -rf -- "$TMP_ROOT"
}
trap cleanup EXIT

for tool in dpkg-deb xvfb-run dbus-run-session curl readlink awk; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "error: packaged Desktop smoke requires $tool" >&2
    exit 1
  }
done
if (( PROOF_MODE )); then
  for tool in jq sha256sum ss git grep setsid stat; do
    command -v "$tool" >/dev/null 2>&1 || {
      echo "error: packaged Desktop proof requires $tool" >&2
      exit 1
    }
  done
fi

deb_path="${1:-}"
if [[ -z "$deb_path" ]]; then
  deb_path="$(find "$DEB_DIR" -maxdepth 1 -name 'Kanban Tool_*.deb' -type f -printf '%T@ %p\n' 2>/dev/null \
    | sort -nr | awk 'NR == 1 { sub(/^[^ ]+ /, ""); print }')"
fi
[[ -n "$deb_path" && -f "$deb_path" ]] || {
  echo "error: no Desktop deb found; run just desktop-package first" >&2
  exit 1
}

mkdir -p "$EXTRACTED" "$RUNTIME_HOME" "$RUNTIME_XDG"
chmod 0700 "$RUNTIME_XDG"
dpkg-deb --extract "$deb_path" "$EXTRACTED"

APP_BINARY="$EXTRACTED/usr/bin/kanban-desktop"
[[ -x "$APP_BINARY" ]] || {
  echo "error: extracted Desktop binary is missing or not executable: $APP_BINARY" >&2
  exit 1
}
mapfile -t desktop_binaries < <(find "$EXTRACTED/usr" -type f -name kanban-desktop -perm /111 -print)
[[ "${#desktop_binaries[@]}" -eq 1 && "${desktop_binaries[0]}" == "$APP_BINARY" ]] || {
  echo "error: extracted Desktop package must contain exactly one usr/bin/kanban-desktop" >&2
  exit 1
}

mapfile -t sidecars < <(find "$EXTRACTED/usr" -type f -name kanban -perm /111 -print)
[[ "${#sidecars[@]}" -eq 1 ]] || {
  echo "error: extracted Desktop package must contain exactly one executable kanban sidecar" >&2
  exit 1
}
SIDECAR_PATH="${sidecars[0]}"
[[ "$SIDECAR_PATH" != "$EXTRACTED/usr/bin/kanban" ]] || {
  echo "error: Desktop sidecar must remain outside /usr/bin" >&2
  exit 1
}

mapfile -t web_manifests < <(find "$EXTRACTED/usr" -type f -path '*/web/manifest.json' -print)
[[ "${#web_manifests[@]}" -eq 1 ]] || {
  echo "error: extracted Desktop package must contain exactly one Web manifest" >&2
  exit 1
}
WEB_MANIFEST_PATH="${web_manifests[0]}"
WEB_ROOT="$(dirname "$WEB_MANIFEST_PATH")"
WEB_BUILD_ID="$(jq -er '.buildId' "$WEB_MANIFEST_PATH")"
DESKTOP_BINARY_SHA256="sha256:$(sha256sum "$APP_BINARY" | awk '{print $1}')"
SIDECAR_SHA256="sha256:$(sha256sum "$SIDECAR_PATH" | awk '{print $1}')"
WEB_MANIFEST_SHA256="sha256:$(sha256sum "$WEB_MANIFEST_PATH" | awk '{print $1}')"

fixed_host_occupied=0
if curl --silent --show-error --max-time 0.5 "http://127.0.0.1:8721/health" >/dev/null 2>&1; then
  fixed_host_occupied=1
fi
if (( PROOF_MODE )) && [[ -n "$(listener_any 8721)" ]]; then
  fixed_host_occupied=1
fi
if [[ "$fixed_host_occupied" -eq 1 ]]; then
  echo "error: fixed Desktop smoke endpoint 127.0.0.1:8721 is already serving a host" >&2
  exit 1
fi

if (( PROOF_MODE )); then
  # 真实初始化隔离 canonical DB，再在 fixed 8721 空闲时验证损坏 manifest 的 Desktop
  # config failure。rollback 不使用脚本自填 fingerprint/seed：before/after 均来自 typed
  # sidecar health/task readback。
  ROLLBACK_ROOT="$TMP_ROOT/rollback-extracted"
  cp -a "$EXTRACTED" "$ROLLBACK_ROOT"
  ROLLBACK_WEB_ROOT="$ROLLBACK_ROOT/${WEB_ROOT#"$EXTRACTED/"}"
  ROLLBACK_APP_BINARY="$ROLLBACK_ROOT/usr/bin/kanban-desktop"
  mapfile -t rollback_sidecars < <(find "$ROLLBACK_ROOT/usr" -type f -name kanban -perm /111 -print)
  [[ "${#rollback_sidecars[@]}" -eq 1 ]] || { echo "error: rollback package copy sidecar missing" >&2; exit 1; }
  ROLLBACK_SIDECAR_PATH="${rollback_sidecars[0]}"
  ROLLBACK_HOME="$TMP_ROOT/rollback-home"
  ROLLBACK_DATA="$TMP_ROOT/rollback-data"
  ROLLBACK_RUNTIME="$TMP_ROOT/rollback-runtime"
  ROLLBACK_DB="$ROLLBACK_DATA/io.github.sockingpanda.kanban/kanban.db"
  ROLLBACK_SNAPSHOT_BEFORE="$TMP_ROOT/rollback-db-before.snapshot"
  ROLLBACK_SNAPSHOT_AFTER="$TMP_ROOT/rollback-db-after.snapshot"
  mkdir -p "$(dirname "$ROLLBACK_DB")" "$ROLLBACK_HOME" "$ROLLBACK_RUNTIME"
  chmod 0700 "$ROLLBACK_RUNTIME"

  [[ -z "$(listener_any "$AUX_PORT")" ]] || {
    echo "error: rollback auxiliary endpoint 127.0.0.1:$AUX_PORT is already occupied" >&2
    exit 1
  }

  start_aux_host "$ROLLBACK_DB" "$WEB_ROOT" "$TMP_ROOT/rollback-seed.stderr" || {
    echo "error: packaged sidecar could not initialize rollback seed host" >&2
    exit 1
  }
  assert_aux_owner || {
    echo "error: rollback auxiliary listener owner changed before typed health read" >&2
    exit 1
  }
  rollback_health_preflight="$(curl --fail --silent --show-error "$AUX_URL/health")"
  [[ "$(jq -er '.data.db' <<<"$rollback_health_preflight")" == "turso" \
    && "$(jq -er '.data.db_path' <<<"$rollback_health_preflight")" == "$ROLLBACK_DB" ]] || {
    echo "error: rollback typed health did not bind Turso DB path before mutation" >&2
    exit 1
  }
  rollback_boards="$(KANBAN_SERVER_URL="$AUX_URL" "$SIDECAR_PATH" --json --board default board list)"
  if ! jq -e '.data | any(.slug == "default")' <<<"$rollback_boards" >/dev/null; then
    KANBAN_SERVER_URL="$AUX_URL" "$SIDECAR_PATH" --json --board default board create default --name "Desktop package rollback" >/dev/null
  fi
  KANBAN_SERVER_URL="$AUX_URL" "$SIDECAR_PATH" --json --board default task create \
    "Desktop package rollback seed" --status todo --task-id t_desktop_package_seed >/dev/null
  assert_aux_owner || {
    echo "error: rollback auxiliary listener owner changed during seed mutation" >&2
    exit 1
  }
  assert_aux_owner || {
    echo "error: rollback auxiliary listener owner changed before seed readback" >&2
    exit 1
  }
  rollback_health_before="$(curl --fail --silent --show-error "$AUX_URL/health")"
  [[ "$(jq -er '.data.db' <<<"$rollback_health_before")" == "turso" \
    && "$(jq -er '.data.db_path' <<<"$rollback_health_before")" == "$ROLLBACK_DB" ]] || {
    echo "error: rollback seeded health did not bind Turso DB path" >&2
    exit 1
  }
  ROLLBACK_FINGERPRINT_BEFORE="$(jq -er '.data.db_fingerprint' <<<"$rollback_health_before")"
  [[ "$ROLLBACK_FINGERPRINT_BEFORE" == turso:* ]] || {
    echo "error: rollback typed health returned a non-Turso fingerprint" >&2
    exit 1
  }
  ROLLBACK_SEED_BEFORE="$(KANBAN_SERVER_URL="$AUX_URL" "$SIDECAR_PATH" --json --board default task show t_desktop_package_seed | jq -er '.data.title')"
  stop_aux_host
  ROLLBACK_IDENTITY_BEFORE="$(stat -c '%d:%i' "$ROLLBACK_DB")"
  ROLLBACK_SHA_BEFORE="sha256:$(sha256sum "$ROLLBACK_DB" | awk '{print $1}')"
  cp -- "$ROLLBACK_DB" "$ROLLBACK_SNAPSHOT_BEFORE"
  chmod 0600 "$ROLLBACK_SNAPSHOT_BEFORE"
  [[ -f "$ROLLBACK_SNAPSHOT_BEFORE" && ! -L "$ROLLBACK_SNAPSHOT_BEFORE" \
    && "$(stat -c '%h' "$ROLLBACK_SNAPSHOT_BEFORE")" == "1" ]] || {
    echo "error: rollback pre-failure DB snapshot must be a private regular file" >&2
    exit 1
  }

  rollback_port_free_before=1
  if curl --silent --max-time 0.5 "http://127.0.0.1:8721/health" >/dev/null 2>&1 \
    || [[ -n "$(listener_any 8721)" ]]; then
    rollback_port_free_before=0
  fi
  RB_MANIFEST_TMP="$TMP_ROOT/rollback-manifest.json.tmp"
  jq '.buildId = "sha256:0000000000000000000000000000000000000000000000000000000000000000"' \
    "$ROLLBACK_WEB_ROOT/manifest.json" > "$RB_MANIFEST_TMP"
  mv -T "$RB_MANIFEST_TMP" "$ROLLBACK_WEB_ROOT/manifest.json"
  env HOME="$ROLLBACK_HOME" XDG_DATA_HOME="$ROLLBACK_DATA" XDG_RUNTIME_DIR="$ROLLBACK_RUNTIME" \
    GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    dbus-run-session -- xvfb-run -a -s "-screen 0 1440x1024x24" \
    "$ROLLBACK_APP_BINARY" >"$TMP_ROOT/rollback.stderr" 2>&1 &
  ROLLBACK_WRAPPER_PID=$!
  rollback_failure_error_observed=0
  rollback_marker_deadline=$((SECONDS + 25))
  while (( SECONDS < rollback_marker_deadline )); do
    if grep -Fq 'kanban Desktop configuration/artifact validation failed:' "$TMP_ROOT/rollback.stderr"; then
      rollback_failure_error_observed=1
      break
    fi
    if ! kill -0 "$ROLLBACK_WRAPPER_PID" 2>/dev/null; then
      break
    fi
    sleep 0.25
  done
  rollback_failure_observed=0
  mapfile -t rollback_live_sidecars < <(rollback_sidecar_pids)
  rollback_sidecar_spawn_zero=0
  [[ "${#rollback_live_sidecars[@]}" -eq 0 ]] && rollback_sidecar_spawn_zero=1
  terminate_exact "$ROLLBACK_APP_BINARY" TERM
  terminate_exact "$ROLLBACK_SIDECAR_PATH" TERM
  for _ in {1..50}; do
    [[ -z "$(rollback_app_pids || true)" && -z "$(rollback_sidecar_pids || true)" ]] && break
    sleep 0.1
  done
  rollback_processes_reaped=0
  [[ -z "$(rollback_app_pids || true)" && -z "$(rollback_sidecar_pids || true)" ]] && rollback_processes_reaped=1
  set +e
  wait "$ROLLBACK_WRAPPER_PID" 2>/dev/null
  rollback_exit_status=$?
  set -e
  rollback_wrapper_pid_observed="$ROLLBACK_WRAPPER_PID"
  ROLLBACK_WRAPPER_PID=""
  rollback_failure_exit_nonzero=0
  [[ "$rollback_exit_status" -ne 0 ]] && rollback_failure_exit_nonzero=1
  rollback_wrapper_reaped=0
  [[ ! -e "/proc/$rollback_wrapper_pid_observed" ]] && rollback_wrapper_reaped=1
  if [[ "$rollback_sidecar_spawn_zero" -eq 1 \
    && "$rollback_failure_error_observed" -eq 1 ]]; then
    rollback_failure_observed=1
  fi
  rollback_port_free_after=1
  if curl --silent --max-time 0.5 "http://127.0.0.1:8721/health" >/dev/null 2>&1 \
    || [[ -n "$(listener_any 8721)" ]]; then
    rollback_port_free_after=0
  fi

  # Capture the post-failure boundary only after the corrupted app, sidecars,
  # wrapper and fixed port have all been reaped, but before any typed readback
  # host reopens the DB.  Readback may refresh metadata/WAL bytes later.
  ROLLBACK_IDENTITY_AFTER="$(stat -c '%d:%i' "$ROLLBACK_DB")"
  ROLLBACK_SHA_AFTER="sha256:$(sha256sum "$ROLLBACK_DB" | awk '{print $1}')"
  cp -- "$ROLLBACK_DB" "$ROLLBACK_SNAPSHOT_AFTER"
  chmod 0600 "$ROLLBACK_SNAPSHOT_AFTER"
  [[ -f "$ROLLBACK_SNAPSHOT_AFTER" && ! -L "$ROLLBACK_SNAPSHOT_AFTER" \
    && "$(stat -c '%h' "$ROLLBACK_SNAPSHOT_AFTER")" == "1" ]] || {
    echo "error: rollback post-failure DB snapshot must be a private regular file" >&2
    exit 1
  }

  start_aux_host "$ROLLBACK_DB" "$WEB_ROOT" "$TMP_ROOT/rollback-readback.stderr" || {
    echo "error: rollback DB readback host failed to start" >&2
    exit 1
  }
  assert_aux_owner || {
    echo "error: rollback auxiliary listener owner changed before typed readback" >&2
    exit 1
  }
  rollback_health_after="$(curl --fail --silent --show-error "$AUX_URL/health")"
  [[ "$(jq -er '.data.db' <<<"$rollback_health_after")" == "turso" \
    && "$(jq -er '.data.db_path' <<<"$rollback_health_after")" == "$ROLLBACK_DB" ]] || {
    echo "error: rollback typed readback did not bind Turso DB path" >&2
    exit 1
  }
  assert_aux_owner || {
    echo "error: rollback auxiliary listener owner changed during typed readback" >&2
    exit 1
  }
  ROLLBACK_FINGERPRINT_AFTER="$(jq -er '.data.db_fingerprint' <<<"$rollback_health_after")"
  [[ "$ROLLBACK_FINGERPRINT_AFTER" == turso:* ]] || {
    echo "error: rollback typed readback returned a non-Turso fingerprint" >&2
    exit 1
  }
  assert_aux_owner || {
    echo "error: rollback auxiliary listener owner changed before readback seed query" >&2
    exit 1
  }
  ROLLBACK_SEED_AFTER="$(KANBAN_SERVER_URL="$AUX_URL" "$SIDECAR_PATH" --json --board default task show t_desktop_package_seed | jq -er '.data.title')"
  stop_aux_host
  rollback_unchanged=0
  [[ "$ROLLBACK_IDENTITY_BEFORE" == "$ROLLBACK_IDENTITY_AFTER" \
    && "$ROLLBACK_SHA_BEFORE" == "$ROLLBACK_SHA_AFTER" \
    && "$ROLLBACK_SEED_BEFORE" == "$ROLLBACK_SEED_AFTER" ]] && rollback_unchanged=1
  [[ "$rollback_failure_observed" -eq 1 \
    && "$rollback_failure_error_observed" -eq 1 \
    && "$rollback_wrapper_reaped" -eq 1 \
    && "$rollback_processes_reaped" -eq 1 \
    && "$rollback_unchanged" -eq 1 \
    && "$rollback_port_free_before" -eq 1 \
    && "$rollback_port_free_after" -eq 1 ]] || {
    echo "error: Desktop rollback failure path did not fail closed" >&2
    printf '  failure_observed=%s failure_error_observed=%s sidecar_spawn_zero=%s failure_exit_nonzero=%s\n' \
      "$rollback_failure_observed" "$rollback_failure_error_observed" \
      "$rollback_sidecar_spawn_zero" "$rollback_failure_exit_nonzero" >&2
    printf '  wrapper_reaped=%s processes_reaped=%s port_free_before=%s port_free_after=%s unchanged=%s\n' \
      "$rollback_wrapper_reaped" "$rollback_processes_reaped" "$rollback_port_free_before" \
      "$rollback_port_free_after" "$rollback_unchanged" >&2
    printf '  identity_before=%s identity_after=%s sha_before=%s sha_after=%s\n' \
      "$ROLLBACK_IDENTITY_BEFORE" "$ROLLBACK_IDENTITY_AFTER" "$ROLLBACK_SHA_BEFORE" "$ROLLBACK_SHA_AFTER" >&2
    printf '  fingerprint_before=%s fingerprint_after=%s seed_before=%s seed_after=%s\n' \
      "$ROLLBACK_FINGERPRINT_BEFORE" "$ROLLBACK_FINGERPRINT_AFTER" "$ROLLBACK_SEED_BEFORE" "$ROLLBACK_SEED_AFTER" >&2
    sed -n '1,120p' "$TMP_ROOT/rollback.stderr" >&2 || true
    exit 1
  }
fi

APP_EXIT_FLAG=1
if (( PROOF_MODE )); then APP_EXIT_FLAG=0; fi
env \
  HOME="$RUNTIME_HOME" \
  XDG_RUNTIME_DIR="$RUNTIME_XDG" \
  GDK_BACKEND=x11 \
  WEBKIT_DISABLE_DMABUF_RENDERER=1 \
  KANBAN_DESKTOP_PACKAGED_SMOKE_EXIT_AFTER_APP_LOAD="$APP_EXIT_FLAG" \
  dbus-run-session -- \
  xvfb-run -a -s "-screen 0 1440x1024x24" \
  "$APP_BINARY" >/dev/null 2>"$TMP_ROOT/desktop.stderr" &
WRAPPER_PID=$!

ready_deadline=$((SECONDS + 25))
page_ready=0
while (( SECONDS < ready_deadline )); do
  if curl --silent --show-error --fail --max-time 0.5 "http://127.0.0.1:8721/health" >/dev/null 2>&1 \
    && curl --silent --show-error --fail --max-time 0.5 "http://127.0.0.1:8721/app/runtime.json" >/dev/null 2>&1 \
    && curl --silent --show-error --fail --max-time 0.5 "http://127.0.0.1:8721/app/manifest.json" >/dev/null 2>&1; then
    mapfile -t owned_sidecars < <(sidecar_pids)
    if [[ "${#owned_sidecars[@]}" -ge 1 ]]; then
      page_ready=1
      break
    fi
  fi
  if ! kill -0 "$WRAPPER_PID" 2>/dev/null; then
    echo "error: packaged Desktop exited before fixed host became ready" >&2
    sed -n '1,120p' "$TMP_ROOT/desktop.stderr" >&2 || true
    exit 1
  fi
  sleep 0.25
done

[[ "$page_ready" -eq 1 ]] || {
  echo "error: packaged Desktop did not expose health/runtime/manifest and owned sidecar within 25s" >&2
  sed -n '1,120p' "$TMP_ROOT/desktop.stderr" >&2 || true
  exit 1
}
mapfile -t owned_sidecars < <(sidecar_pids)
[[ "${#owned_sidecars[@]}" -ge 1 ]] || {
  echo "error: packaged Desktop did not spawn its owned kanban sidecar" >&2
  sed -n '1,120p' "$TMP_ROOT/desktop.stderr" >&2 || true
  exit 1
}

if (( ! PROOF_MODE )); then
  cleanup_deadline=$((SECONDS + 12))
  while (( SECONDS < cleanup_deadline )); do
    app_pid="$(app_pid_for_path || true)"
    mapfile -t remaining_sidecars < <(sidecar_pids)
    host_alive=0
    if curl --silent --max-time 0.5 "http://127.0.0.1:8721/health" >/dev/null 2>&1; then host_alive=1; fi
    wrapper_alive=0
    wrapper_state="$(awk '{print $3}' "/proc/$WRAPPER_PID/stat" 2>/dev/null || true)"
    if kill -0 "$WRAPPER_PID" 2>/dev/null && [[ "$wrapper_state" != "Z" ]]; then wrapper_alive=1; fi
    if [[ -z "$app_pid" && "${#remaining_sidecars[@]}" -eq 0 && "$host_alive" -eq 0 && "$wrapper_alive" -eq 0 ]]; then break; fi
    sleep 0.1
  done
  if [[ -n "$app_pid" || "${#remaining_sidecars[@]}" -ne 0 || "$host_alive" -ne 0 || "$wrapper_alive" -ne 0 ]]; then
    echo "error: packaged Desktop cleanup did not converge within 12s" >&2
    exit 1
  fi
  set +e
  wait "$WRAPPER_PID" 2>/dev/null
  wrapper_status=$?
  set -e
  WRAPPER_PID=""
  if [[ "$wrapper_status" -ne 0 ]]; then
    echo "error: packaged Desktop wrapper exited with status $wrapper_status" >&2
    sed -n '1,120p' "$TMP_ROOT/desktop.stderr" >&2 || true
    exit 1
  fi
  echo "ok: packaged Desktop WebKitGTK smoke passed for $deb_path"
  exit 0
fi

live_app_pid="$(app_pid_for_path || true)"
[[ -n "$live_app_pid" ]] || { echo "error: proof could not find live Desktop PID" >&2; exit 1; }
live_app_identity="${live_app_pid}:$(awk '{print $22}' "/proc/$live_app_pid/stat")"
live_app_argv="$(tr '\0' '\n' < "/proc/$live_app_pid/cmdline" | jq -Rsc 'split("\n") | map(select(length > 0))')"
live_sidecar_pids_json="$(printf '%s\n' "${owned_sidecars[@]}" | jq -Rsc 'split("\n") | map(select(length > 0) | tonumber)')"
live_sidecar_identities_json='[]'
live_sidecar_argv_json='[]'
for pid in "${owned_sidecars[@]}"; do
  identity="${pid}:$(awk '{print $22}' "/proc/$pid/stat")"
  argv="$(tr '\0' '\n' < "/proc/$pid/cmdline" | jq -Rsc 'split("\n") | map(select(length > 0))')"
  live_sidecar_identities_json="$(jq -c --arg value "$identity" '. + [$value]' <<<"$live_sidecar_identities_json")"
  live_sidecar_argv_json="$(jq -c --argjson value "$argv" '. + [$value]' <<<"$live_sidecar_argv_json")"
done
port_owner_pids_json="$(ss -H -ltnp 'sport = :8721' 2>/dev/null | grep -o 'pid=[0-9]*' | cut -d= -f2 | sort -nu | jq -Rsc 'split("\n") | map(select(length > 0) | tonumber)')"
[[ "$port_owner_pids_json" != "[]" ]] || { echo "error: proof could not identify fixed-port listener PID" >&2; exit 1; }

curl --silent --show-error --fail "http://127.0.0.1:8721/health" > "$TMP_ROOT/health.json"
curl --silent --show-error --fail "http://127.0.0.1:8721/app/runtime.json" > "$TMP_ROOT/runtime.json"
curl --silent --show-error --fail "http://127.0.0.1:8721/app/manifest.json" > "$TMP_ROOT/live-manifest.json"
RUNTIME_SHA256="sha256:$(sha256sum "$TMP_ROOT/runtime.json" | awk '{print $1}')"
MANIFEST_LIVE_SHA256="sha256:$(sha256sum "$TMP_ROOT/live-manifest.json" | awk '{print $1}')"
RUNTIME_WEB_BUILD_ID="$(jq -er '.webBuildId' "$TMP_ROOT/runtime.json")"
MANIFEST_BUILD_ID="$(jq -er '.buildId' "$TMP_ROOT/live-manifest.json")"
HEALTH_DB_FINGERPRINT="$(jq -er '.data.db_fingerprint' "$TMP_ROOT/health.json")"
HEALTH_VERSION="$(jq -er '.data.version' "$TMP_ROOT/health.json")"
PAYLOADS_JSON="$(jq -c '[.files[] | {path, sha256, bytes}]' "$WEB_MANIFEST_PATH")"

prepare_evidence_destination
tmp_evidence="$EVIDENCE_PATH.tmp.$$"
[[ ! -e "$tmp_evidence" && ! -L "$tmp_evidence" ]] || { echo "error: evidence temp path exists" >&2; exit 1; }
jq -n \
  --arg run_id "$RUN_ID" \
  --arg start_sha "$START_SHA" \
  --argjson start_clean "$([[ "$START_CLEAN" -eq 1 ]] && echo true || echo false)" \
  --arg deb_path "$(readlink -f "$deb_path")" \
  --arg deb_sha256 "sha256:$(sha256sum "$deb_path" | awk '{print $1}')" \
  --arg extracted_root "$EXTRACTED" \
  --arg desktop_binary_path "$APP_BINARY" \
  --arg desktop_binary_sha256 "$DESKTOP_BINARY_SHA256" \
  --arg sidecar_path "$SIDECAR_PATH" \
  --arg sidecar_sha256 "$SIDECAR_SHA256" \
  --arg web_root "$WEB_ROOT" \
  --arg web_manifest_path "$WEB_MANIFEST_PATH" \
  --arg web_manifest_sha256 "$WEB_MANIFEST_SHA256" \
  --arg runtime_sha256 "$RUNTIME_SHA256" \
  --arg manifest_live_sha256 "$MANIFEST_LIVE_SHA256" \
  --arg web_build_id "$WEB_BUILD_ID" \
  --arg runtime_web_build_id "$RUNTIME_WEB_BUILD_ID" \
  --arg manifest_build_id "$MANIFEST_BUILD_ID" \
  --arg health_db_fingerprint "$HEALTH_DB_FINGERPRINT" \
  --arg health_version "$HEALTH_VERSION" \
  --argjson app_pid "$live_app_pid" \
  --arg app_identity "$live_app_identity" \
  --argjson app_argv "$live_app_argv" \
  --arg app_exe_sha256 "$DESKTOP_BINARY_SHA256" \
  --argjson sidecar_pids "$live_sidecar_pids_json" \
  --argjson sidecar_identities "$live_sidecar_identities_json" \
  --argjson sidecar_argv "$live_sidecar_argv_json" \
  --arg sidecar_exe_sha256 "$SIDECAR_SHA256" \
  --argjson port_owner_pids "$port_owner_pids_json" \
  --arg rollback_db_path "$ROLLBACK_DB" \
  --arg rollback_snapshot_before "$ROLLBACK_SNAPSHOT_BEFORE" \
  --arg rollback_snapshot_after "$ROLLBACK_SNAPSHOT_AFTER" \
  --arg rollback_identity_before "$ROLLBACK_IDENTITY_BEFORE" \
  --arg rollback_identity_after "$ROLLBACK_IDENTITY_AFTER" \
  --arg rollback_sha_before "$ROLLBACK_SHA_BEFORE" \
  --arg rollback_sha_after "$ROLLBACK_SHA_AFTER" \
  --arg rollback_fingerprint_before "$ROLLBACK_FINGERPRINT_BEFORE" \
  --arg rollback_fingerprint_after "$ROLLBACK_FINGERPRINT_AFTER" \
  --arg rollback_seed_before "$ROLLBACK_SEED_BEFORE" \
  --arg rollback_seed_after "$ROLLBACK_SEED_AFTER" \
  --argjson rollback_failure_observed "$([[ "$rollback_failure_observed" -eq 1 ]] && echo true || echo false)" \
  --argjson rollback_failure_exit_nonzero "$([[ "$rollback_failure_exit_nonzero" -eq 1 ]] && echo true || echo false)" \
  --argjson rollback_failure_error_observed "$([[ "$rollback_failure_error_observed" -eq 1 ]] && echo true || echo false)" \
  --argjson rollback_sidecar_spawn_zero "$([[ "$rollback_sidecar_spawn_zero" -eq 1 ]] && echo true || echo false)" \
  --argjson rollback_wrapper_reaped "$([[ "$rollback_wrapper_reaped" -eq 1 ]] && echo true || echo false)" \
  --argjson rollback_processes_reaped "$([[ "$rollback_processes_reaped" -eq 1 ]] && echo true || echo false)" \
  --argjson rollback_unchanged "$([[ "$rollback_unchanged" -eq 1 ]] && echo true || echo false)" \
  --argjson rollback_port_free_before "$([[ "$rollback_port_free_before" -eq 1 ]] && echo true || echo false)" \
  --argjson rollback_port_free_after "$([[ "$rollback_port_free_after" -eq 1 ]] && echo true || echo false)" \
  --argjson payloads "$PAYLOADS_JSON" \
  '{schema_version:1,run_id:$run_id,start_sha:$start_sha,start_clean:$start_clean,deb_path:$deb_path,deb_sha256:$deb_sha256,extracted_root:$extracted_root,desktop_binary_path:$desktop_binary_path,desktop_binary_sha256:$desktop_binary_sha256,sidecar_path:$sidecar_path,sidecar_sha256:$sidecar_sha256,web_root:$web_root,web_manifest_path:$web_manifest_path,web_manifest_sha256:$web_manifest_sha256,runtime_sha256:$runtime_sha256,manifest_live_sha256:$manifest_live_sha256,web_build_id:$web_build_id,runtime_web_build_id:$runtime_web_build_id,manifest_build_id:$manifest_build_id,payloads:$payloads,live:{base_url:"http://127.0.0.1:8721",health_checked:true,runtime_checked:true,manifest_checked:true,health_db_fingerprint:$health_db_fingerprint,health_version:$health_version,runtime_web_build_id:$runtime_web_build_id,manifest_build_id:$manifest_build_id,runtime_sha256:$runtime_sha256,manifest_sha256:$manifest_live_sha256,app_pid:$app_pid,app_identity:$app_identity,app_argv:$app_argv,app_exe_sha256:$app_exe_sha256,sidecar_pids:$sidecar_pids,sidecar_identities:$sidecar_identities,sidecar_argv:$sidecar_argv,sidecar_exe_sha256:$sidecar_exe_sha256,port_owner_pids:$port_owner_pids},rollback:{failure_observed:$rollback_failure_observed,failure_exit_nonzero:$rollback_failure_exit_nonzero,failure_error_observed:$rollback_failure_error_observed,sidecar_spawn_zero:$rollback_sidecar_spawn_zero,db_path:$rollback_db_path,db_snapshot_before_path:$rollback_snapshot_before,db_snapshot_after_path:$rollback_snapshot_after,db_identity_before:$rollback_identity_before,db_identity_after:$rollback_identity_after,db_sha256_before:$rollback_sha_before,db_sha256_after:$rollback_sha_after,db_fingerprint_before:$rollback_fingerprint_before,db_fingerprint_after:$rollback_fingerprint_after,seed_before:$rollback_seed_before,seed_after:$rollback_seed_after,unchanged:$rollback_unchanged,wrapper_reaped:$rollback_wrapper_reaped,processes_reaped:$rollback_processes_reaped,port_free_before:$rollback_port_free_before,port_free_after:$rollback_port_free_after}}' \
  > "$tmp_evidence"
chmod 0600 "$tmp_evidence"
[[ -f "$tmp_evidence" && ! -L "$tmp_evidence" && "$(stat -c '%h' "$tmp_evidence")" == "1" ]] || {
  echo "error: Desktop package evidence temp must be a private regular file" >&2
  exit 1
}
mv -T "$tmp_evidence" "$EVIDENCE_PATH"
[[ -f "$EVIDENCE_PATH" && ! -L "$EVIDENCE_PATH" \
  && "$(stat -c '%a' "$EVIDENCE_PATH")" == "600" \
  && "$(stat -c '%h' "$EVIDENCE_PATH")" == "1" ]] || {
  echo "error: Desktop package evidence destination must be private regular file" >&2
  exit 1
}

if [[ "$SKIP_RECEIPT" != "1" ]]; then
  scripts/cargo-build-lock.sh -- cargo run --locked -p xtask --bin xtask -- \
    release package --root "$ROOT" --evidence "$EVIDENCE_PATH" --out "$RECEIPT_PATH"
else
  scripts/cargo-build-lock.sh -- cargo run --locked -p xtask --bin xtask -- \
    release package --diagnostic --root "$ROOT" --evidence "$EVIDENCE_PATH"
fi

proof_wrapper_pid="$WRAPPER_PID"
cleanup_runtime
final_host_alive=0
if curl --silent --max-time 0.5 "http://127.0.0.1:8721/health" >/dev/null 2>&1; then final_host_alive=1; fi
port_free=1
if [[ "$final_host_alive" -eq 1 || -n "$(listener_any 8721)" ]]; then port_free=0; fi
aux_port_free=1
if curl --silent --max-time 0.5 "$AUX_URL/health" >/dev/null 2>&1 \
  || [[ -n "$(listener_any "$AUX_PORT")" ]]; then
  aux_port_free=0
fi
remaining_app="$(executable_pids "$APP_BINARY" || true)"
remaining_sidecars="$(executable_pids "$SIDECAR_PATH" || true)"
remaining_rollback_app="$(executable_pids "$ROLLBACK_APP_BINARY" || true)"
remaining_rollback_sidecar="$(executable_pids "$ROLLBACK_SIDECAR_PATH" || true)"
remaining_wrapper=0
if [[ -n "$proof_wrapper_pid" && -e "/proc/$proof_wrapper_pid" ]]; then remaining_wrapper=1; fi
remaining_rollback_wrapper=0
if [[ -n "${rollback_wrapper_pid_observed:-}" && -e "/proc/$rollback_wrapper_pid_observed" ]]; then
  remaining_rollback_wrapper=1
fi
[[ "$port_free" -eq 1 \
  && "$aux_port_free" -eq 1 \
  && -z "$remaining_app" \
  && -z "$remaining_sidecars" \
  && -z "$remaining_rollback_app" \
  && -z "$remaining_rollback_sidecar" \
  && "$remaining_wrapper" -eq 0 \
  && "$remaining_rollback_wrapper" -eq 0 \
  && -z "$(listener_any 8721)" \
  && -z "$(listener_any "$AUX_PORT")" ]] || {
  echo "error: Desktop package cleanup left app/sidecar/wrapper/port resources" >&2
  exit 1
}
if [[ "$SKIP_RECEIPT" == "1" ]]; then
  echo "ok: packaged Desktop diagnostic proof passed for $deb_path; evidence=$EVIDENCE_PATH"
else
  echo "ok: packaged Desktop proof passed for $deb_path; receipt=$RECEIPT_PATH"
fi
