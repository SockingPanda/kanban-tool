#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
HOST_PID=""
SERVER_URL=""
TARGET_DIR=""
KANBAN_BIN=""

for tool in curl jq node; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "error: $tool is required" >&2
    exit 1
  }
done

SMOKE_DIR="$(mktemp -d)"

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
  node -e '
    const net = require("node:net");
    const server = net.createServer();
    server.on("error", error => {
      console.error(error);
      process.exitCode = 1;
    });
    server.listen(0, "127.0.0.1", () => {
      process.stdout.write(`${server.address().port}\n`);
      server.close();
    });
  '
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
    if curl --silent --show-error --fail --max-time 0.5 "http://127.0.0.1:$port/health" 2>/dev/null |
      jq -e --arg expected_db "$expected_db" '
        .data.ok == true
        and .data.db == "turso"
        and .data.db_path == $expected_db
      ' >/dev/null
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
      --host 127.0.0.1 --port "$port" --no-web "${dispatcher_args[@]}"
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
task_id="$(jq -r '.data.id' <<<"$task_json")"
kb task step not-required "$task_id" --reason "smoke task has no execution plan steps" >/dev/null
kb task promote "$task_id" >/dev/null

for _ in {1..120}; do
  task_status="$(kb task show "$task_id" | jq -r '.data.status')"
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
run_id="$(jq -r '.data[0].id' <<<"$runs_json")"
logs_json="$(kb run logs "$run_id")"
jq -e '.data.content | contains("smoke log")' <<<"$logs_json" >/dev/null

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
jq -e '.data.phase == "completed" and .data.restart_required == false' <<<"$import_json" >/dev/null
imported_task_json="$(kb task show "$task_id")"
jq -e --arg task_id "$task_id" '.data.id == $task_id and .data.status == "done"' \
  <<<"$imported_task_json" >/dev/null

echo "v1 本地单 Host smoke 通过：$ROOT"
