#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_DIR="$(mktemp -d)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
HOST_PID=""
SERVER_URL=""
TARGET_DIR=""
KANBAN_BIN=""

cleanup_host() {
  if [[ -z "$HOST_PID" ]]; then
    return 0
  fi

  # 只向本次启动的 host PID 发信号，不扫描或终止其他进程。
  if kill -0 "$HOST_PID" >/dev/null 2>&1; then
    kill -TERM "$HOST_PID" >/dev/null 2>&1 || true
    for _ in {1..100}; do
      if ! kill -0 "$HOST_PID" >/dev/null 2>&1; then
        break
      fi
      sleep 0.02
    done
    if kill -0 "$HOST_PID" >/dev/null 2>&1; then
      kill -KILL "$HOST_PID" >/dev/null 2>&1 || true
    fi
  fi
  wait "$HOST_PID" >/dev/null 2>&1 || true
  HOST_PID=""
}

cleanup() {
  cleanup_host
  rm -rf "$SMOKE_DIR"
}
trap cleanup EXIT

TARGET_DIR="$("$LOCK" --print-target-dir)"
"$LOCK" -- cargo build -q --manifest-path "$ROOT/Cargo.toml" -p kanban-cli --bin kanban
KANBAN_BIN="$TARGET_DIR/debug/kanban"
[[ -x "$KANBAN_BIN" ]] || {
  echo "错误：未找到 kanban CLI binary：$KANBAN_BIN" >&2
  exit 1
}

free_loopback_port() {
  python3 - <<'PY'
import socket

with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
    sock.bind(("127.0.0.1", 0))
    print(sock.getsockname()[1])
PY
}

wait_for_health() {
  local host_pid="$1"
  local port="$2"
  local expected_db="$3"
  local deadline=$((SECONDS + 120))

  while (( SECONDS < deadline )); do
    if ! kill -0 "$host_pid" >/dev/null 2>&1; then
      echo "错误：kanban serve 在健康检查前退出；日志如下：" >&2
      sed -n '1,240p' "$SMOKE_DIR/serve.log" >&2 || true
      return 1
    fi
    if python3 - "$port" "$expected_db" <<'PY'
import json
import sys
import urllib.request

try:
    with urllib.request.urlopen(
        f"http://127.0.0.1:{int(sys.argv[1])}/health", timeout=0.5
    ) as response:
        payload = json.load(response)
    report = payload.get("data", {})
    if (
        response.status == 200
        and report.get("ok") is True
        and report.get("db") == "turso"
        and report.get("db_path") == sys.argv[2]
    ):
        raise SystemExit(0)
except (OSError, ValueError, AttributeError, TypeError, json.JSONDecodeError):
    pass
raise SystemExit(1)
PY
    then
      return 0
    fi
    sleep 0.1
  done

  echo "错误：kanban serve 未能在预期时间内健康监听 127.0.0.1:$port；日志如下：" >&2
  sed -n '1,240p' "$SMOKE_DIR/serve.log" >&2 || true
  return 1
}

start_host() {
  local db_path="$1"
  local dispatcher_profile="${2:-}"
  local port
  local dispatcher_args=()
  port="$(free_loopback_port)"
  SERVER_URL="http://127.0.0.1:$port"
  if [[ -n "$dispatcher_profile" ]]; then
    dispatcher_args=(--dispatcher-profile "$dispatcher_profile")
  fi

  cleanup_host
  (
    cd "$SMOKE_DIR"
    exec env -u KANBAN_DB -u KB_DB -u KANBAN_SERVER_URL \
      "$KANBAN_BIN" \
      --db "$db_path" --actor smoke-host serve \
      --host 127.0.0.1 --port "$port" "${dispatcher_args[@]}"
  ) >"$SMOKE_DIR/serve.log" 2>&1 &
  HOST_PID=$!
  wait_for_health "$HOST_PID" "$port" "$db_path"
}

kb() {
  # 所有产品命令都显式指向当前 host；不提供 --db，也清除可能触发本地配置的环境变量。
  (
    cd "$SMOKE_DIR"
    env -u KANBAN_DB -u KB_DB -u KANBAN_SERVER_URL -u KB_BOARD \
      XDG_CONFIG_HOME="$SMOKE_DIR/xdg-config" XDG_DATA_HOME="$SMOKE_DIR/xdg-data" \
      "$KANBAN_BIN" \
      --server-url "$SERVER_URL" --board default --actor smoke --json "$@"
  )
}

cat >"$SMOKE_DIR/dispatcher.toml" <<EOF
board = "default"
command = "printf 'smoke log\\n'"
poll_interval_ms = 20
claim_ttl_ms = 10000
heartbeat_interval_ms = 1000
on_success = "done"
on_failure = "blocked"
log_dir = "logs"
EOF

# `dispatch` 语义由 canonical host 的 dispatcher profile 提供，只会原子 claim `ready`。
start_host "$SMOKE_DIR/kb.db" "$SMOKE_DIR/dispatcher.toml"

kb init >/dev/null
kb board list >/dev/null

task_json="$(kb task create "v1 smoke task" --description "ready spec" --status todo --max-retries 2)"
task_id="$(printf '%s' "$task_json" | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["id"])')"
kb task step not-required "$task_id" --reason "smoke task has no execution plan steps" >/dev/null
kb task promote "$task_id" >/dev/null

for _ in {1..120}; do
  task_status="$(kb task show "$task_id" | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["status"])')"
  if [[ "$task_status" == "done" ]]; then
    break
  fi
  sleep 0.1
done
[[ "$task_status" == "done" ]] || {
  echo "错误：dispatcher 未能完成 smoke task（status=$task_status）" >&2
  exit 1
}

runs_json="$(kb runs "$task_id")"
run_id="$(printf '%s' "$runs_json" | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"][0]["id"])')"
logs_json="$(kb run logs "$run_id")"
printf '%s' "$logs_json" | python3 -c '
import json,sys
payload=json.load(sys.stdin)
assert "smoke log" in payload["data"]["content"]
'

# dispatcher 完成后释放其持续轮询；维护操作由同一 canonical DB 的无 dispatcher host 执行。
cleanup_host
start_host "$SMOKE_DIR/kb.db"
kb stats >/dev/null
kb doctor >/dev/null
kb checkpoint >/dev/null
kb vacuum >/dev/null
kb backup --path "$SMOKE_DIR/backup.sqlite" >/dev/null
kb export --path "$SMOKE_DIR/board.jsonl" >/dev/null

# portable import 需要新的空 canonical host；先停止旧 host，再切换数据库并由新 host 导入。
cleanup_host
start_host "$SMOKE_DIR/imported.db"
import_json="$(kb import --path "$SMOKE_DIR/board.jsonl")"
printf '%s' "$import_json" | python3 -c '
import json,sys
report=json.load(sys.stdin)["data"]
assert report["phase"] == "completed"
assert report["restart_required"] is False
'
imported_task_json="$(kb task show "$task_id")"
printf '%s' "$imported_task_json" | python3 -c '
import json
import sys

task = json.load(sys.stdin)["data"]
assert task["id"] == sys.argv[1]
assert task["status"] == "done"
' "$task_id"

echo "v1 本地单 Host smoke 通过：$ROOT"
