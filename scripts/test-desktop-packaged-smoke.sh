#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
DEB_DIR="$("$LOCK" --print-target-dir)/release/bundle/deb"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/kanban-desktop-smoke.XXXXXX")"
EXTRACTED="$TMP_ROOT/extracted"
RUNTIME_HOME="$TMP_ROOT/home"
RUNTIME_XDG="$TMP_ROOT/xdg-runtime"
WRAPPER_PID=""
SIDECAR_PATH=""

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

cleanup() {
  set +e
  terminate_exact() {
    local expected="$1" signal="$2" pid
    [[ -n "$expected" ]] || return 0
    while read -r pid; do
      [[ -n "$pid" ]] && kill -s "$signal" "$pid" 2>/dev/null || true
    done < <(executable_pids "$expected")
  }
  terminate_exact "${APP_BINARY:-}" TERM
  terminate_exact "${SIDECAR_PATH:-}" TERM
  if [[ -n "$WRAPPER_PID" ]] && kill -0 "$WRAPPER_PID" 2>/dev/null; then
    kill -TERM "$WRAPPER_PID" 2>/dev/null || true
  fi
  for _ in {1..40}; do
    local app_left sidecar_left
    app_left="$(executable_pids "${APP_BINARY:-}" || true)"
    sidecar_left="$(executable_pids "${SIDECAR_PATH:-}" || true)"
    [[ -z "$app_left" && -z "$sidecar_left" ]] && break
    sleep 0.1
  done
  terminate_exact "${APP_BINARY:-}" KILL
  terminate_exact "${SIDECAR_PATH:-}" KILL
  if [[ -n "$WRAPPER_PID" ]]; then
    wait "$WRAPPER_PID" 2>/dev/null || true
  fi
  rm -rf -- "$TMP_ROOT"
}
trap cleanup EXIT

for tool in dpkg-deb xvfb-run dbus-run-session curl readlink awk; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "error: packaged Desktop smoke requires $tool" >&2
    exit 1
  }
done

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

# smoke 不应 attach 到无关 host；测试期间由 Desktop 独占 fixed endpoint，预先占用必须失败。
if curl --silent --show-error --max-time 0.5 "http://127.0.0.1:8721/health" >/dev/null 2>&1; then
  echo "error: fixed Desktop smoke endpoint 127.0.0.1:8721 is already serving a host" >&2
  exit 1
fi

env \
  HOME="$RUNTIME_HOME" \
  XDG_RUNTIME_DIR="$RUNTIME_XDG" \
  GDK_BACKEND=x11 \
  WEBKIT_DISABLE_DMABUF_RENDERER=1 \
  KANBAN_DESKTOP_PACKAGED_SMOKE_EXIT_AFTER_APP_LOAD=1 \
  dbus-run-session -- \
  xvfb-run -a -s "-screen 0 1440x1024x24" \
  "$APP_BINARY" >/dev/null 2>"$TMP_ROOT/desktop.stderr" &
WRAPPER_PID=$!

ready_deadline=$((SECONDS + 25))
page_ready=""
while (( SECONDS < ready_deadline )); do
  if curl --silent --show-error --fail --max-time 0.5 "http://127.0.0.1:8721/health" >/dev/null 2>&1 \
    && curl --silent --show-error --fail --max-time 0.5 "http://127.0.0.1:8721/app/runtime.json" >/dev/null 2>&1; then
    page_ready=1
    mapfile -t owned_sidecars < <(sidecar_pids)
    if [[ "${#owned_sidecars[@]}" -ge 1 ]]; then
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

[[ -n "$page_ready" ]] || {
  echo "error: packaged Desktop did not expose /health and /app/runtime.json within 25s" >&2
  sed -n '1,120p' "$TMP_ROOT/desktop.stderr" >&2 || true
  exit 1
}

mapfile -t owned_sidecars < <(sidecar_pids)
[[ "${#owned_sidecars[@]}" -ge 1 ]] || {
  echo "error: packaged Desktop did not spawn its owned kanban sidecar" >&2
  exit 1
}

# strict opt-in page-load hook 经 Tauri 正常 ExitRequested 路径退出；等待 bounded graceful ->
# force cleanup 移除 app、fixed host 和 exact owned sidecar，不向无关进程发送信号。
cleanup_deadline=$((SECONDS + 12))
while (( SECONDS < cleanup_deadline )); do
  app_pid="$(app_pid_for_path || true)"
  mapfile -t remaining_sidecars < <(sidecar_pids)
  host_alive=0
  if curl --silent --max-time 0.5 "http://127.0.0.1:8721/health" >/dev/null 2>&1; then
    host_alive=1
  fi
  wrapper_alive=0
  wrapper_state="$(awk '{print $3}' "/proc/$WRAPPER_PID/stat" 2>/dev/null || true)"
  if kill -0 "$WRAPPER_PID" 2>/dev/null && [[ "$wrapper_state" != "Z" ]]; then
    wrapper_alive=1
  fi
  if [[ -z "$app_pid" && "${#remaining_sidecars[@]}" -eq 0 && "$host_alive" -eq 0 && "$wrapper_alive" -eq 0 ]]; then
    break
  fi
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
