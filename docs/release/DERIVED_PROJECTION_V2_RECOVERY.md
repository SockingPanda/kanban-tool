# Derived Projection v2 生产恢复 runbook

本 runbook 只适用于一个 SQLite 数据库对应的一套 DB-scoped Projection v2。SQLite 是唯一
canonical truth；Tantivy、Oxigraph、LanceDB task chunks 和 LanceDB label atoms 都是可删除、
可重建的派生层。所有 canonical 写入必须经过 CLI/service path；本文件不授权直接 SQL
写入，也不把派生层故障当作回滚 canonical mutation 的理由。

本流程只采用已经由 `just release` 发布、且带有同级
`<generation>.published` marker 的完整 release cohort。运行本流程前，值班人员必须在
同一个受控 shell 中依次执行所有代码块；不要把函数或变量拆到状态不明的第二个 shell。

本 runbook 的命令示例是 Linux/Debian 生产操作程序（例如 `systemctl`、`fuser`、GNU
`stat` 与文件租约）；它不构成 Windows runtime 已在本机验证的证据。Windows
`LockFileEx`/per-store range-lock 路径必须由独立的 native 环境与 reviewer 按
`just check-windows-p kanban-local` 等门禁另行验收；在没有该证据时不得把 Windows
标为“本机已验证”，也不得以本 runbook 的 Linux 结果替代该验收。

## 1. 硬停止条件和禁止事项

以下任一项不满足时，保持生产只读，不执行 rebuild、legacy cleanup、服务替换、canary
mutation 或 owner 启动：

- 工作树、实时 `origin/main`、release cohort 和实际部署 artifact 的
  commit/tree/build identity/hash 尚未精确一致；
- 代码、故障矩阵、Windows durability、schema、release provenance 或独立 native review
  仍有 P0/P1/P2；
- 没有停止清单中的全部 writer unit 和非 systemd writer，或 singleton/store lease 仍有效；
- 还没有成功 checkpoint、canonical SQLite backup、backup hash 和隔离 restore drill；
- database identity、Projection protocol、四个 store 的 schema/provider/fingerprint/corpus
  binding 或 DB migration 不兼容；
- 无法唯一确认 SQLite 路径、writer inventory、运行用户、managed-data 路径、rollback
  目录或同文件系统边界；
- 存在未知 dirty 工作树、未知 maintenance owner、无法解释的 generation/journal、
  非零 checkpoint busy、磁盘空间不足或不受保护的 backup；
- continuous owner 不能证明实际拥有
  `tantivy_tasks`、`oxigraph_relations`、`lancedb_chunks`、
  `lancedb_label_atoms` 四项 capability。

永远不要：

- 清空 `index_outbox` / `projection_deliveries`，伪造 checkpoint、watermark、coverage、
  fingerprint 或 generation；
- 手工修改 dirty、error、generation、provider/corpus binding、lease/fence 或
  `projection_*` 控制字段；
- 轮流写 Tantivy v1，或把 v1 layout 当作 v2 active generation；
- 直接写 `tasks.status`、`label_semantics`、`label_atoms`、`task_labels` 或 outbox；
- 删除 WAL/SHM、journal、previous generation、cleanup backup 或 canonical backup；
- 把 `previous_generation` 当作存在一个可调用的 previous-generation republish API。

## 2. 固定变量、私有证据根和 writer inventory

变量必须来自已复核的部署记录，不能根据默认路径猜测。`WRITER_UNITS` 必须列出会写这个
SQLite 或其派生目录的全部 systemd unit，包括 maintenance、API/serve、Desktop embedded
server、import/replace/vacuum/replacement job；`WRITER_PIDS` 与
`WRITER_PROCESS_EXES` 一一对应，只列出经过 `/proc/<pid>/exe` 复核的非 systemd writer。
没有非 systemd writer 时两个数组都保持为空。runbook 会在任何 stop 前保存每个存活 PID
的原始 start-time；后续只有 start-time → executable → start-time 三者仍精确一致时才
允许发送信号。

`EXPECTED_BINDINGS_SOURCE` 是在恢复前由发布/config owner 审批的 TSV，不能从当前 status
反向生成。`VECTOR_CONFIG` 为空时使用部署的正常本地配置；非空时必须是同一 release/config
记录中的绝对路径。所有 `systemctl`、CLI、SQLite、helper 和 root-side `/proc`/signal
调用都经过同一个 GNU `timeout` wall-clock 边界；`rc=124` 表示 TERM timeout，
`rc=137` 表示 timeout 后 KILL，其他非零值保持原命令失败语义。所有提权调用固定使用
`sudo -n`，绝不等待口令或其它交互输入。

~~~bash
set -Eeuo pipefail
umask 077

export RECOVERY_ID='<UTC-timestamp>-<main-commit>'
export REPO='<absolute-clean-main-worktree>'
export DB='<canonical-absolute-SQLite-path>'
export EVIDENCE='<new-absolute-recovery-evidence-directory>'
export COHORT='<absolute-published-release-generation-directory>'
export KANBAN_BIN='<absolute-deployed-kanban-binary>'
export MAINTENANCE_UNIT='<verified-continuous-maintenance-systemd-unit>'
export VECTOR_HELPER='<absolute-deployed-kanban-vector-lancedb>'
export GRAPH_HELPER='<absolute-deployed-kanban-graph-oxigraph>'
export VECTOR_CONFIG=''
export EXPECTED_BINDINGS_SOURCE='<absolute-approved-active-bindings-tsv>'
export POLL_INTERVAL_SEC=5
export SLA_TIMEOUT_SEC=120
export STOP_LEASE_TIMEOUT_SEC=120
export OWNER_START_TIMEOUT_SEC=120
export EXTERNAL_COMMAND_TIMEOUT_SEC=20
export EXTERNAL_KILL_AFTER_SEC=5

export KANBAN_VECTOR_HELPER="$VECTOR_HELPER"
export KANBAN_GRAPH_HELPER="$GRAPH_HELPER"

declare -a WRITER_UNITS=(
  "$MAINTENANCE_UNIT"
  '<verified-API-or-serve-unit>'
  '<verified-Desktop-embedded-server-unit>'
)
declare -a WRITER_PIDS=()
declare -a WRITER_PROCESS_EXES=()
declare -A WRITER_START_TIMES=()

for required_path in \
  "$REPO" "$DB" "$COHORT" "$KANBAN_BIN" "$VECTOR_HELPER" "$GRAPH_HELPER" \
  "$EXPECTED_BINDINGS_SOURCE"
do
  case "$required_path" in
    /*) ;;
    *) printf 'not an absolute path: %s\n' "$required_path" >&2; exit 1 ;;
  esac
done
test -d "$REPO"
test -f "$DB"
test -d "$COHORT"
test -x "$KANBAN_BIN"
test -x "$VECTOR_HELPER"
test -x "$GRAPH_HELPER"
test -f "$EXPECTED_BINDINGS_SOURCE"
test "${#WRITER_UNITS[@]}" -ge 1
test "${WRITER_UNITS[0]}" = "$MAINTENANCE_UNIT"
test "${#WRITER_PIDS[@]}" -eq "${#WRITER_PROCESS_EXES[@]}"
if test "${#WRITER_PIDS[@]}" -gt 0; then
  test "$(printf '%s\n' "${WRITER_PIDS[@]}" | sort -u | wc -l)" \
    -eq "${#WRITER_PIDS[@]}"
fi
test "$(printf '%s\n' "${WRITER_UNITS[@]}" | sort -u | wc -l)" \
  -eq "${#WRITER_UNITS[@]}"
for pid in "${WRITER_PIDS[@]}"; do
  [[ "$pid" =~ ^[1-9][0-9]*$ ]]
done
for exe in "${WRITER_PROCESS_EXES[@]}"; do
  case "$exe" in
    /*) ;;
    *) printf 'writer exe is not absolute: %s\n' "$exe" >&2; exit 1 ;;
  esac
done
for seconds in "$EXTERNAL_COMMAND_TIMEOUT_SEC" "$EXTERNAL_KILL_AFTER_SEC"; do
  [[ "$seconds" =~ ^[1-9][0-9]*$ ]]
done
for required_command in timeout sudo python3 systemctl sqlite3 flock; do
  command -v "$required_command" >/dev/null
done

bounded_external() {
  command timeout --signal=TERM \
    --kill-after="${EXTERNAL_KILL_AFTER_SEC}s" \
    "${EXTERNAL_COMMAND_TIMEOUT_SEC}s" "$@"
}

bounded_sudo() {
  bounded_external sudo -n "$@"
}

bounded_kanban() {
  bounded_external "$KANBAN_BIN" "$@"
}

bounded_sqlite() {
  bounded_external sqlite3 "$@"
}

bounded_vector_helper() {
  bounded_external "$VECTOR_HELPER" "$@"
}

bounded_graph_helper() {
  bounded_external "$GRAPH_HELPER" "$@"
}

evidence_write_failed=0
record_evidence_write_failure() {
  evidence_write_failed=1
  hard_stop_evidence_failed=1 2>/dev/null || true
  printf 'evidence write failed: %s\n' "$1" >&2
}

evidence_truncate() {
  if ! : >"$1"; then
    record_evidence_write_failure "$1"
    return 1
  fi
}

evidence_append() {
  local evidence_file="$1"; shift
  if ! printf "$@" >>"$evidence_file"; then
    record_evidence_write_failure "$evidence_file"
    return 1
  fi
}

evidence_capture() {
  local evidence_file="$1"; shift
  if "$@" >"$evidence_file"; then
    return 0
  fi
  local rc=$?
  record_evidence_write_failure "$evidence_file"
  return "$rc"
}

external_rc_class() {
  case "$1" in
    0) printf 'ok\n' ;;
    124) printf 'timeout\n' ;;
    137) printf 'timeout-killed\n' ;;
    *) printf 'exit\n' ;;
  esac
}

test ! -e "$EVIDENCE"
install -d -m 0700 "$EVIDENCE"
for subdir in \
  preflight checkpoint backup restore-drill compatibility canaries queries sla restart final
do
  install -d -m 0700 "$EVIDENCE/$subdir"
done
test "$(stat -c '%a' "$EVIDENCE")" = 700

# 在任何 stop/start/mutation 之前建立并持有跨 subshell 的 guard；guard 初始化失败时
# 立即 hard-stop，不能进入生产动作。后续 evidence 目录即使不可写，已持有的 lock
# 仍用于 parent/subshell 的一次性协调。
export HARD_STOP_GUARD_FILE="$EVIDENCE/sla/.hard-stop.guard"
if ! exec {HARD_STOP_GUARD_FD}>"$HARD_STOP_GUARD_FILE"; then
  printf 'hard-stop guard initialization failed: %s\n' "$HARD_STOP_GUARD_FILE" >&2
  exit 1
fi
if ! flock -n "$HARD_STOP_GUARD_FD"; then
  printf 'hard-stop guard lock acquisition failed: %s\n' "$HARD_STOP_GUARD_FILE" >&2
  exit 1
fi
export HARD_STOP_OWNER_BASHPID="$BASHPID"
~~~

只保存 unit 的非秘密属性；`Environment` 原文只保留在 shell 变量中，先筛选为两个 helper
变量再落盘。这个检查证明 unit 配置绑定，而 owner 启动后的 `/proc/<MainPID>/environ`
检查才是实际进程绑定的最终证明。

~~~bash
for unit in "${WRITER_UNITS[@]}"; do
  safe_name="${unit//[^A-Za-z0-9_.@-]/_}"
  bounded_sudo systemctl show "$unit" \
    --property=FragmentPath,ExecStart,User,MainPID,ActiveState,SubState \
    >"$EVIDENCE/preflight/unit.$safe_name.show.txt"
  bounded_sudo systemctl is-enabled "$unit" \
    >"$EVIDENCE/preflight/unit.$safe_name.enabled.txt"
done

assert_configured_unit_binding() {
  local raw_environment
  raw_environment="$(
    bounded_sudo systemctl show "$MAINTENANCE_UNIT" \
      --property=Environment --value
  )"
  UNIT_ENV_RAW="$raw_environment" bounded_external python3 - \
    "$KANBAN_VECTOR_HELPER" "$KANBAN_GRAPH_HELPER" \
    >"$EVIDENCE/preflight/maintenance-unit.helper-environment.redacted.json" <<'PY'
import json
import os
import shlex
import sys

expected = {
    "KANBAN_VECTOR_HELPER": sys.argv[1],
    "KANBAN_GRAPH_HELPER": sys.argv[2],
}
actual = {}
for item in shlex.split(os.environ["UNIT_ENV_RAW"]):
    if "=" in item:
        key, value = item.split("=", 1)
        if key in expected:
            actual[key] = value
if actual != expected:
    raise SystemExit(
        "maintenance unit helper Environment does not exactly match deployed helpers"
    )
print(json.dumps({"selected_environment": actual, "exact_match": True}, sort_keys=True))
PY
  unset UNIT_ENV_RAW
}
~~~

以下函数是全流程唯一的 writer stop/assertion 实现。`systemctl stop` 后把 failed state
复位并要求精确 `inactive`。对每个仍存活的非 systemd PID，先在 stop 前捕获
`/proc/<pid>/stat` 的 start-time；每次发送 `TERM` 或 `KILL` 前都按
start-time → executable → start-time 的顺序重新读取并要求两次 start-time、捕获值和
预期 executable 全部一致。PID existence 只由同一个 root-side probe 判定为
`alive`/`absent`/`error`：只有 `kill(2)` 返回 `ESRCH` 且 `/proc/<pid>` 确认不存在才是
`absent`；`sudo`、`EPERM`、timeout、输出异常或其它执行错误一律是 `error`，记录
no-signal 并 fail closed。identity 采样期间 PID 消失、被复用或字段漂移也绝不向该 PID
发信号。

~~~bash
root_pid_state() {
  local pid="$1" output rc
  if ! [[ "$pid" =~ ^[1-9][0-9]*$ ]]; then
    printf 'error\n'
    return 2
  fi
  if output="$(
    bounded_sudo python3 -c '
import errno
import os
import pathlib
import sys

pid = int(sys.argv[1])
proc = pathlib.Path(f"/proc/{pid}")

def proc_presence():
    try:
        os.stat(proc)
    except FileNotFoundError:
        return "absent"
    except OSError:
        return "error"
    return "present"

try:
    os.kill(pid, 0)
except ProcessLookupError:
    if proc_presence() == "absent":
        print("absent")
        raise SystemExit(0)
    print("error")
    raise SystemExit(3)
except PermissionError:
    print("error")
    raise SystemExit(3)
except OSError as error:
    if error.errno == errno.ESRCH and proc_presence() == "absent":
        print("absent")
        raise SystemExit(0)
    print("error")
    raise SystemExit(3)
else:
    if proc_presence() == "present":
        print("alive")
        raise SystemExit(0)
    print("error")
    raise SystemExit(3)
' "$pid" 2>/dev/null
  )"; then
    rc=0
  else
    rc=$?
  fi
  if test "$rc" -eq 0 &&
    { test "$output" = alive || test "$output" = absent; }; then
    printf '%s\n' "$output"
    return 0
  fi
  printf 'error\n'
  if test "$rc" -eq 0; then
    return 2
  fi
  return "$rc"
}

process_start_time() {
  local pid="$1" stat_line stat_tail rc
  local -a stat_fields
  if stat_line="$(bounded_sudo cat "/proc/$pid/stat" 2>/dev/null)"; then
    :
  else
    rc=$?
    return "$rc"
  fi
  stat_tail="${stat_line##*) }"
  read -r -a stat_fields <<<"$stat_tail"
  if test "${#stat_fields[@]}" -lt 20 ||
    ! [[ "${stat_fields[19]}" =~ ^[0-9]+$ ]]; then
    return 1
  fi
  printf '%s\n' "${stat_fields[19]}"
}

assert_same_process_identity() {
  local pid="$1" expected_exe="$2" captured_start="$3"
  local start_before actual_exe start_after rc
  if start_before="$(process_start_time "$pid")"; then
    :
  else
    rc=$?
    return "$rc"
  fi
  if actual_exe="$(
    bounded_sudo readlink -f "/proc/$pid/exe" 2>/dev/null
  )"; then
    :
  else
    rc=$?
    return "$rc"
  fi
  if start_after="$(process_start_time "$pid")"; then
    :
  else
    rc=$?
    return "$rc"
  fi
  if test "$start_before" != "$captured_start" ||
    test "$start_after" != "$captured_start" ||
    test "$actual_exe" != "$expected_exe"; then
    return 1
  fi
}

signal_same_process() {
  local signal="$1" pid="$2" expected_exe="$3" captured_start="$4"
  local state probe_rc identity_rc signal_rc
  case "$signal" in
    TERM|KILL) ;;
    *) printf 'unsupported signal: %s\n' "$signal" >&2; return 1 ;;
  esac
  if state="$(root_pid_state "$pid")"; then
    probe_rc=0
  else
    probe_rc=$?
    return "$probe_rc"
  fi
  case "$state" in
    absent)
      printf 'absent-no-signal\n'
      return 0
      ;;
    alive) ;;
    *) return 2 ;;
  esac
  if assert_same_process_identity \
    "$pid" "$expected_exe" "$captured_start"; then
    :
  else
    identity_rc=$?
    case "$identity_rc" in
      124|137) return "$identity_rc" ;;
      *) return 2 ;;
    esac
  fi
  if bounded_sudo kill "-$signal" "$pid"; then
    printf 'alive-signaled\n'
    return 0
  else
    signal_rc=$?
    return "$signal_rc"
  fi
}

assert_writers_stopped() {
  local evidence_file="${1:-$EVIDENCE/preflight/writers.assert-stopped.txt}"
  local unit state pid rc pid_state probe_rc failed=0 timeout_rc=0
  evidence_truncate "$evidence_file" || failed=1
  for unit in "${WRITER_UNITS[@]}"; do
    if state="$(bounded_sudo systemctl is-active "$unit" 2>&1)"; then
      rc=0
    else
      rc=$?
    fi
    printf 'unit=%s state=%s rc=%s rc_class=%s\n' \
      "$unit" "$state" "$rc" "$(external_rc_class "$rc")" \
      >>"$evidence_file" || record_evidence_write_failure "$evidence_file"
    if test "$state" != inactive || test "$rc" -ne 3; then
      failed=1
      case "$rc" in
        124|137) test "$timeout_rc" -ne 0 || timeout_rc="$rc" ;;
      esac
    fi
  done
  for pid in "${WRITER_PIDS[@]}"; do
    if pid_state="$(root_pid_state "$pid")"; then
      probe_rc=0
    else
      probe_rc=$?
      pid_state=error
    fi
    printf 'pid=%s state=%s probe_rc=%s probe_rc_class=%s\n' \
      "$pid" "$pid_state" "$probe_rc" "$(external_rc_class "$probe_rc")" \
      >>"$evidence_file" || record_evidence_write_failure "$evidence_file"
    if test "$pid_state" != absent; then
      failed=1
      case "$probe_rc" in
        124|137) test "$timeout_rc" -ne 0 || timeout_rc="$probe_rc" ;;
      esac
    fi
  done
  if test "$timeout_rc" -ne 0; then
    return "$timeout_rc"
  fi
  return "$failed"
}

stop_all_writers() {
  local index pid expected captured_start deadline kill_deadline pid_state probe_rc
  local signal_result
  bounded_sudo systemctl stop "${WRITER_UNITS[@]}"
  for unit in "${WRITER_UNITS[@]}"; do
    bounded_sudo systemctl reset-failed "$unit"
  done
  for index in "${!WRITER_PIDS[@]}"; do
    pid="${WRITER_PIDS[$index]}"
    expected="${WRITER_PROCESS_EXES[$index]}"
    if pid_state="$(root_pid_state "$pid")"; then
      probe_rc=0
    else
      probe_rc=$?
      return "$probe_rc"
    fi
    case "$pid_state" in
      absent) ;;
      alive)
        captured_start="${WRITER_START_TIMES[$pid]-}"
        test -n "$captured_start"
        if signal_result="$(signal_same_process TERM "$pid" "$expected" "$captured_start")"; then
          case "$signal_result" in
            alive-signaled|absent-no-signal) ;;
            *) return 1 ;;
          esac
        else
          return $?
        fi
        ;;
      *) return 1 ;;
    esac
  done

  deadline=$(( $(date +%s) + 30 ))
  for index in "${!WRITER_PIDS[@]}"; do
    pid="${WRITER_PIDS[$index]}"
    expected="${WRITER_PROCESS_EXES[$index]}"
    captured_start="${WRITER_START_TIMES[$pid]-}"
    while :; do
      if pid_state="$(root_pid_state "$pid")"; then
        probe_rc=0
      else
        probe_rc=$?
        return "$probe_rc"
      fi
      case "$pid_state" in
        absent) break ;;
        alive) ;;
        *) return 1 ;;
      esac
      if test "$(date +%s)" -ge "$deadline"; then
        test -n "$captured_start"
        if signal_result="$(signal_same_process KILL "$pid" "$expected" "$captured_start")"; then
          case "$signal_result" in
            alive-signaled|absent-no-signal) ;;
            *) return 1 ;;
          esac
        else
          return $?
        fi
        break
      fi
      sleep 1
    done
  done
  kill_deadline=$(( $(date +%s) + 10 ))
  for pid in "${WRITER_PIDS[@]}"; do
    while :; do
      if pid_state="$(root_pid_state "$pid")"; then
        probe_rc=0
      else
        probe_rc=$?
        return "$probe_rc"
      fi
      case "$pid_state" in
        absent) break ;;
        alive) ;;
        *) return 1 ;;
      esac
      test "$(date +%s)" -lt "$kill_deadline"
      sleep 1
    done
  done
  assert_writers_stopped
}

: >"$EVIDENCE/preflight/non-systemd-writers.captured.txt"
for index in "${!WRITER_PIDS[@]}"; do
  pid="${WRITER_PIDS[$index]}"
  expected="${WRITER_PROCESS_EXES[$index]}"
  if pid_state="$(root_pid_state "$pid")"; then
    probe_rc=0
  else
    probe_rc=$?
    pid_state=error
  fi
  case "$pid_state" in
    alive)
      captured_start="$(process_start_time "$pid")"
      actual="$(bounded_sudo readlink -f "/proc/$pid/exe")"
      test "$(process_start_time "$pid")" = "$captured_start"
      test "$actual" = "$expected"
      WRITER_START_TIMES["$pid"]="$captured_start"
      printf 'pid=%s exe=%s start_time=%s state=captured\n' \
        "$pid" "$actual" "$captured_start" \
        >>"$EVIDENCE/preflight/non-systemd-writers.captured.txt"
      ;;
    absent)
      printf 'pid=%s exe=%s state=absent\n' "$pid" "$expected" \
        >>"$EVIDENCE/preflight/non-systemd-writers.captured.txt"
      ;;
    *)
      if test "$probe_rc" -eq 0; then
        probe_rc=2
      fi
      printf 'pid=%s exe=%s state=error probe_rc=%s probe_rc_class=%s\n' \
        "$pid" "$expected" "$probe_rc" "$(external_rc_class "$probe_rc")" \
        >>"$EVIDENCE/preflight/non-systemd-writers.captured.txt"
      exit "$probe_rc"
      ;;
  esac
done

assert_no_database_holders() {
  local evidence_file="${1:-$EVIDENCE/preflight/database-holders.after-stop.txt}"
  local output status sidecar
  local -a targets=("$DB")
  command -v fuser >/dev/null
  for sidecar in "$DB-wal" "$DB-shm" "$DB-journal"; do
    if test -e "$sidecar"; then
      targets+=("$sidecar")
    fi
  done
  if output="$(bounded_sudo fuser "${targets[@]}" 2>&1)"; then
    status=0
  else
    status=$?
  fi
  if ! printf '%s\n' "$output" >"$evidence_file"; then
    record_evidence_write_failure "$evidence_file"
    return 1
  fi
  if test "$status" -eq 1 && test -z "$output"; then
    return 0
  fi
  case "$status" in
    0|1) return 1 ;;
    *) return "$status" ;;
  esac
}

wait_for_lease_expiry() {
  local deadline now_ms rc
  deadline=$(( $(date +%s) + STOP_LEASE_TIMEOUT_SEC ))
  while :; do
    if evidence_capture "$EVIDENCE/preflight/maintenance-status.waiting-for-expiry.json" \
      bounded_kanban --db "$DB" --json maintenance status; then
      :
    else
      rc=$?
      return "$rc"
    fi
    now_ms=$(( $(date +%s) * 1000 ))
    if jq -e --argjson now "$now_ms" '
      .data.maintenance_owner.active == false and
      all(.data.stores[];
        (.owner == null) or
        (.lease_expires_at != null and .lease_expires_at <= $now)
      )
    ' "$EVIDENCE/preflight/maintenance-status.waiting-for-expiry.json" >/dev/null; then
      break
    fi
    test "$(date +%s)" -lt "$deadline"
    sleep 1
  done
  if ! cp --no-preserve=mode \
    "$EVIDENCE/preflight/maintenance-status.waiting-for-expiry.json" \
    "$EVIDENCE/preflight/maintenance-status.leases-expired.json"; then
    record_evidence_write_failure \
      "$EVIDENCE/preflight/maintenance-status.leases-expired.json"
    return 1
  fi
  if ! jq -e --argjson now "$(( $(date +%s) * 1000 ))" '
    .data.maintenance_owner.active == false and
    all(.data.stores[];
      (.owner == null) or
      (.lease_expires_at != null and .lease_expires_at <= $now)
    )
  ' "$EVIDENCE/preflight/maintenance-status.leases-expired.json" \
    >"$EVIDENCE/preflight/leases-expired.ok.txt"; then
    record_evidence_write_failure "$EVIDENCE/preflight/leases-expired.ok.txt"
    return 1
  fi
}

wait_for_exact_maintenance_release() {
  local output_dir="$1" phase="$2" deadline current rc
  install -d -m 0700 "$output_dir" || return 1
  current="$output_dir/$phase.maintenance-status.current.json"
  deadline=$(( $(date +%s) + STOP_LEASE_TIMEOUT_SEC ))
  while :; do
    if evidence_capture "$current" bounded_kanban --db "$DB" --json maintenance status; then
      :
    else
      rc=$?
      return "$rc"
    fi
    if jq -e '
      .data.maintenance_owner.active == false and
      .data.maintenance_owner.owner == null and
      .data.maintenance_owner.mode == null and
      .data.maintenance_owner.capabilities == [] and
      .data.maintenance_owner.build_identity == null and
      .data.maintenance_owner.lease_expires_at == null and
      .data.maintenance_owner.last_heartbeat_at == null and
      (.data.stores | length) == 4 and
      ([.data.stores[].store_name] | sort) ==
        ["lancedb_chunks","lancedb_label_atoms",
         "oxigraph_relations","tantivy_tasks"] and
      all(.data.stores[];
        .owner == null and .lease_expires_at == null
      )
    ' "$current" >/dev/null; then
      break
    fi
    if test "$(date +%s)" -ge "$deadline"; then
      if ! cp --no-preserve=mode "$current" \
        "$output_dir/$phase.maintenance-status.timeout.json" 2>/dev/null; then
        record_evidence_write_failure "$output_dir/$phase.maintenance-status.timeout.json"
      fi
      return 1
    fi
    sleep 1 || return 1
  done
  if ! cp --no-preserve=mode "$current" \
    "$output_dir/$phase.maintenance-status.released.json"; then
    record_evidence_write_failure "$output_dir/$phase.maintenance-status.released.json"
    return 1
  fi
  if ! jq -e '
    .data.maintenance_owner.active == false and
    .data.maintenance_owner.owner == null and
    .data.maintenance_owner.mode == null and
    .data.maintenance_owner.capabilities == [] and
    .data.maintenance_owner.build_identity == null and
    .data.maintenance_owner.lease_expires_at == null and
    .data.maintenance_owner.last_heartbeat_at == null and
    all(.data.stores[]; .owner == null and .lease_expires_at == null)
  ' "$output_dir/$phase.maintenance-status.released.json" \
    >"$output_dir/$phase.exact-idle.ok.txt"; then
    record_evidence_write_failure "$output_dir/$phase.exact-idle.ok.txt"
    return 1
  fi
}
~~~

## 3. release marker、provenance、artifact 和部署身份

`${COHORT}.published` 是权威 marker；`$COHORT/.published` 不是合法路径。下面的函数同时
验证 clean symbolic main、实时远端、source provenance、artifact manifest、marker/tree、
cohort 中六个 artifact，以及实际部署的 CLI/两个 helper。它必须在任何 canonical mutation
前运行一次，并在部署后、最终验收时原样重跑。

~~~bash
assert_release_binding() {
  local phase="$1" dir commit tree branch remote_output remote_commit remote_ref
  local source artifacts marker source_sha source_map_sha build_id generation
  local index role relative expected_sha expected_size actual_path actual_sha actual_size
  local -a roles relatives deployed

  dir="$EVIDENCE/preflight/release-$phase"
  install -d -m 0700 "$dir"
  source="$COHORT/source-provenance.json"
  artifacts="$COHORT/release-artifacts.json"
  marker="${COHORT}.published"
  test -s "$source"
  test -s "$artifacts"
  test -f "$marker"
  test ! -L "$marker"

  branch="$(git -C "$REPO" symbolic-ref --quiet HEAD)"
  test "$branch" = refs/heads/main
  git -C "$REPO" status --porcelain=v1 --untracked-files=all \
    >"$dir/git-status.porcelain.txt"
  test ! -s "$dir/git-status.porcelain.txt"
  commit="$(git -C "$REPO" rev-parse --verify 'HEAD^{commit}')"
  tree="$(git -C "$REPO" rev-parse --verify 'HEAD^{tree}')"
  [[ "$commit" =~ ^[0-9a-f]{40}$ ]]
  [[ "$tree" =~ ^[0-9a-f]{40}$ ]]
  printf '%s\n' "$commit" >"$dir/git.commit.txt"
  printf '%s\n' "$tree" >"$dir/git.tree.txt"

  remote_output="$(
    git -C "$REPO" ls-remote --exit-code origin refs/heads/main
  )"
  printf '%s\n' "$remote_output" >"$dir/remote-main.txt"
  test "$(printf '%s\n' "$remote_output" | awk 'NF { count++ } END { print count+0 }')" -eq 1
  read -r remote_commit remote_ref extra <<<"$remote_output"
  test -z "${extra:-}"
  test "$remote_ref" = refs/heads/main
  test "$remote_commit" = "$commit"

  jq -e --arg commit "$commit" --arg tree "$tree" '
    .schema_version == 3 and .project == "kanban-tool" and
    .branch == "main" and .commit == $commit and .tree == $tree and
    .remote.name == "origin" and .remote.ref == "refs/heads/main" and
    .remote.commit == $commit and
    .identity_sha256 == .identity.identity_sha256 and
    .generation_key == ($commit + "-" + $tree + "-" + .identity_sha256) and
    .build_id ==
      ("kanban-tool/" + .version + ";commit=" + $commit +
       ";tree=" + $tree + ";identity=" + .identity_sha256)
  ' "$source" >"$dir/source-provenance.ok.txt"
  "$REPO/scripts/release-source-gate.sh" validate --manifest "$source"
  printf 'source_gate_validate=true\n' \
    >"$dir/source-provenance.schema-and-identity.ok.txt"

  source_sha="$(sha256sum "$source" | awk '{print $1}')"
  source_map_sha="$(sha256sum "$COHORT/derived-projection-v2-source-map.json" | awk '{print $1}')"
  jq -e --slurpfile source "$source" \
    --arg source_sha "$source_sha" --arg source_map_sha "$source_map_sha" '
    ($source | length) == 1 and
    .schema_version == 3 and .project == "kanban-tool" and
    .commit == $source[0].commit and .tree == $source[0].tree and
    .version == $source[0].version and
    .build_id == $source[0].build_id and
    .generation_key == $source[0].generation_key and
    .identity_sha256 == $source[0].identity_sha256 and
    .identity == $source[0].identity and
    .source_manifest.sha256 == $source_sha and
    .source_map.sha256 == $source_map_sha and
    .source_map.sha256 == $source[0].source_map.sha256 and
    (.artifacts | length) == 6 and
    ([.artifacts[].role] | sort) ==
      ["cli_binary","cli_deb","desktop_binary","desktop_deb",
       "lancedb_helper","oxigraph_helper"] and
    all(.artifacts[]; .build_id == $source[0].build_id) and
    (.artifacts | map({key:.role,value:.path}) | from_entries) as $paths |
    $paths.cli_binary == (.generation_path + "/artifacts/bin/kanban") and
    $paths.lancedb_helper ==
      (.generation_path + "/artifacts/bin/kanban-vector-lancedb") and
    $paths.oxigraph_helper ==
      (.generation_path + "/artifacts/bin/kanban-graph-oxigraph") and
    $paths.desktop_binary ==
      (.generation_path + "/artifacts/bin/kanban-desktop") and
    $paths.cli_deb ==
      (.generation_path + "/artifacts/deb/kanban-tool-cli.deb") and
    $paths.desktop_deb ==
      (.generation_path + "/artifacts/deb/kanban-tool-desktop.deb")
  ' "$artifacts" >"$dir/release-artifacts.ok.txt"

  generation="$(jq -r '.generation_key' "$source")"
  build_id="$(jq -r '.build_id' "$source")"
  test "$(basename "$COHORT")" = "$generation"
  bounded_external python3 "$REPO/scripts/release-safe-path.py" validate-published-dir \
    --root "$(dirname "$COHORT")" --path "$COHORT" --marker "$marker"
  printf 'marker=%s\nvalidated=true\n' "$marker" \
    >"$dir/published-marker-tree.ok.txt"
  sha256sum "$source" "$artifacts" \
    "$COHORT/derived-projection-v2-source-map.json" "$marker" \
    >"$dir/cohort-control-files.sha256"

  roles=(
    cli_binary lancedb_helper oxigraph_helper
    desktop_binary cli_deb desktop_deb
  )
  relatives=(
    artifacts/bin/kanban
    artifacts/bin/kanban-vector-lancedb
    artifacts/bin/kanban-graph-oxigraph
    artifacts/bin/kanban-desktop
    artifacts/deb/kanban-tool-cli.deb
    artifacts/deb/kanban-tool-desktop.deb
  )
  deployed=(
    "$KANBAN_BIN" "$VECTOR_HELPER" "$GRAPH_HELPER" "" "" ""
  )
  printf 'role\tcohort_path\tsha256\tsize\tdeployed_path\n' \
    >"$dir/artifact-bindings.tsv"
  for index in "${!roles[@]}"; do
    role="${roles[$index]}"
    relative="${relatives[$index]}"
    actual_path="$COHORT/$relative"
    test -f "$actual_path"
    test ! -L "$actual_path"
    expected_sha="$(
      jq -er --arg role "$role" '.artifacts[] | select(.role == $role) | .sha256' \
        "$artifacts"
    )"
    expected_size="$(
      jq -er --arg role "$role" '.artifacts[] | select(.role == $role) | .size' \
        "$artifacts"
    )"
    actual_sha="$(sha256sum "$actual_path" | awk '{print $1}')"
    actual_size="$(stat -c '%s' "$actual_path")"
    test "$actual_sha" = "$expected_sha"
    test "$actual_size" = "$expected_size"
    if test -n "${deployed[$index]}"; then
      test -f "${deployed[$index]}"
      test "$(sha256sum "${deployed[$index]}" | awk '{print $1}')" = "$expected_sha"
      test "$(stat -c '%s' "${deployed[$index]}")" = "$expected_size"
    fi
    printf '%s\t%s\t%s\t%s\t%s\n' \
      "$role" "$actual_path" "$actual_sha" "$actual_size" "${deployed[$index]}" \
      >>"$dir/artifact-bindings.tsv"
  done

  bounded_vector_helper __build-identity \
    >"$dir/vector-helper.build-identity.txt"
  bounded_graph_helper __build-identity \
    >"$dir/graph-helper.build-identity.txt"
  test "$(tr -d '\n' <"$dir/vector-helper.build-identity.txt")" = "$build_id"
  test "$(tr -d '\n' <"$dir/graph-helper.build-identity.txt")" = "$build_id"
  bounded_kanban --version >"$dir/kanban.version.txt"
  printf '%s\n' "$build_id" >"$dir/expected-build-identity.txt"
}

assert_release_binding pre-mutation
~~~

在停止 writer 之前保存只读基线。此处只要求 canonical doctor 字段通过；派生层仍可处于
待恢复状态，所以不能用当前 derived readiness 反向定义预期 binding。

~~~bash
bounded_kanban --db "$DB" --json maintenance status \
  >"$EVIDENCE/preflight/maintenance-status.before-stop.json"
bounded_kanban --db "$DB" --json doctor \
  >"$EVIDENCE/preflight/doctor.before-stop.json"
bounded_kanban --db "$DB" --json outbox list --limit 1000 \
  >"$EVIDENCE/preflight/outbox.before-stop.json"
jq -e '
  .data.integrity_check == "ok" and
  .data.migration_version == 30 and .data.user_version == 30 and
  .data.consistency_errors == 0 and
  .data.ontology_ledger_errors == 0
' "$EVIDENCE/preflight/doctor.before-stop.json" \
  >"$EVIDENCE/preflight/doctor.canonical.before-stop.ok.txt"
~~~

## 4. 停止全部 writer、lease expiry、checkpoint 和 backup

先停止 inventory 中的精确 unit/PID，再证明数据库没有遗留 holder，最后等待 singleton owner
和四个 store lease 失效。整个 dry-run、deploy 和 rebuild 期间，这些 unit/PID 都必须保持
停止；只允许当前 shell 启动的定向 `maintenance rebuild` 一次性 owner。

~~~bash
stop_all_writers
assert_no_database_holders
wait_for_lease_expiry
assert_writers_stopped

bounded_kanban --db "$DB" --json maintenance status \
  >"$EVIDENCE/preflight/maintenance-status.after-stop.json"
bounded_kanban --db "$DB" --json doctor \
  >"$EVIDENCE/preflight/doctor.after-stop.json"
bounded_kanban --db "$DB" --json outbox list --limit 1000 \
  >"$EVIDENCE/preflight/outbox.after-stop.json"
~~~

用正常 CLI checkpoint，硬性要求 `busy=0`。失败时不要删除 WAL/SHM。

~~~bash
bounded_kanban --db "$DB" --json checkpoint \
  | tee "$EVIDENCE/checkpoint/checkpoint.json"
jq -e '.data.busy == 0' "$EVIDENCE/checkpoint/checkpoint.json" \
  >"$EVIDENCE/checkpoint/checkpoint.busy.ok.txt"

for sidecar in "$DB-wal" "$DB-shm" "$DB-journal"; do
  if test -e "$sidecar"; then
    stat --printf='%n size=%s inode=%i mtime=%y\n' "$sidecar"
  else
    printf '%s absent\n' "$sidecar"
  fi
done >"$EVIDENCE/checkpoint/sidecars.stat.txt"
~~~

checkpoint 成功后才创建新的 canonical backup。backup 和 hash 均明确收紧为 `0600`；
`kanban backup` 使用 `VACUUM INTO`，不会覆盖既有文件。

~~~bash
test ! -e "$EVIDENCE/backup/canonical.sqlite"
bounded_kanban --db "$DB" --json backup \
  --out "$EVIDENCE/backup/canonical.sqlite" \
  | tee "$EVIDENCE/backup/backup.json"
chmod 0600 "$EVIDENCE/backup/canonical.sqlite"
(
  cd "$EVIDENCE/backup"
  sha256sum canonical.sqlite >canonical.sqlite.sha256
)
chmod 0600 "$EVIDENCE/backup/canonical.sqlite.sha256"
(
  cd "$EVIDENCE/backup"
  sha256sum --check canonical.sqlite.sha256
) | tee "$EVIDENCE/backup/canonical.sqlite.sha256.check.txt"
bounded_sqlite -readonly "$EVIDENCE/backup/canonical.sqlite" \
  'PRAGMA integrity_check;' \
  | tee "$EVIDENCE/backup/integrity_check.txt"
test "$(tr -d '[:space:]' <"$EVIDENCE/backup/integrity_check.txt")" = ok
test "$(stat -c '%a' "$EVIDENCE/backup/canonical.sqlite")" = 600
test "$(stat -c '%a' "$EVIDENCE/backup/canonical.sqlite.sha256")" = 600
stat --printf='%n size=%s inode=%i mtime=%y filesystem=%m\n' \
  "$DB" "$EVIDENCE/backup/canonical.sqlite" \
  >"$EVIDENCE/backup/files.stat.txt"
df -P "$DB" "$EVIDENCE/backup/canonical.sqlite" \
  >"$EVIDENCE/backup/filesystem.stat.txt"
~~~

backup 后、任何 rebuild 前继续使用只读 legacy inventory；保存精确 digest。非空
WAL/journal 会令 inventory fail closed；不要删 sidecar，也不要提前调用 cleanup apply。

~~~bash
bounded_kanban --db "$DB" --json maintenance cleanup-legacy inventory \
  | tee "$EVIDENCE/preflight/legacy-inventory.json"
LEGACY_DIGEST="$(
  jq -er '.data.inventory_digest | select(length > 0)' \
    "$EVIDENCE/preflight/legacy-inventory.json"
)"
printf '%s\n' "$LEGACY_DIGEST" \
  >"$EVIDENCE/preflight/legacy-inventory.digest.txt"
assert_writers_stopped
~~~

## 5. 隔离 restore drill

restore drill 只在 `EVIDENCE/restore-drill` 中进行，绝不替换生产 DB。它验证 backup checksum、
doctor、database identity 和九个 active board 的 canonical stats。

~~~bash
export DRILL_DB="$EVIDENCE/restore-drill/kb.db"
test ! -e "$DRILL_DB"
cp --reflink=auto --no-preserve=mode \
  "$EVIDENCE/backup/canonical.sqlite" "$DRILL_DB"
chmod 0600 "$DRILL_DB"
(
  cd "$EVIDENCE/restore-drill"
  sha256sum kb.db >kb.db.sha256
)
chmod 0600 "$EVIDENCE/restore-drill/kb.db.sha256"
test "$(awk '{print $1}' "$EVIDENCE/backup/canonical.sqlite.sha256")" = \
  "$(awk '{print $1}' "$EVIDENCE/restore-drill/kb.db.sha256")"

bounded_kanban --db "$DRILL_DB" --json doctor \
  | tee "$EVIDENCE/restore-drill/doctor.json"
bounded_kanban --db "$DRILL_DB" --json maintenance status \
  | tee "$EVIDENCE/restore-drill/maintenance-status.json"
bounded_kanban --db "$DRILL_DB" --json outbox list --limit 1000 \
  >"$EVIDENCE/restore-drill/outbox.json"
jq -e '
  .data.integrity_check == "ok" and
  .data.migration_version == 30 and .data.user_version == 30 and
  .data.consistency_errors == 0 and
  .data.ontology_ledger_errors == 0
' "$EVIDENCE/restore-drill/doctor.json" \
  >"$EVIDENCE/restore-drill/doctor.canonical.ok.txt"

bounded_kanban --db "$DB" --json board list --include-archived \
  | tee "$EVIDENCE/restore-drill/boards.production.json"
jq -e '[.data[] | select(.archived_at == null)] | length == 9' \
  "$EVIDENCE/restore-drill/boards.production.json" \
  >"$EVIDENCE/restore-drill/boards.count.ok.txt"
jq -r '.data[] | select(.archived_at == null) | [.slug,.id] | @tsv' \
  "$EVIDENCE/restore-drill/boards.production.json" \
  >"$EVIDENCE/restore-drill/boards.tsv"

while IFS=$'\t' read -r BOARD BOARD_ID; do
  bounded_kanban --db "$DB" --board "$BOARD" --json stats \
    >"$EVIDENCE/restore-drill/stats.production.$BOARD.json"
  bounded_kanban --db "$DRILL_DB" --board "$BOARD" --json stats \
    >"$EVIDENCE/restore-drill/stats.restore.$BOARD.json"
  jq 'del(.data.generated_at)' \
    "$EVIDENCE/restore-drill/stats.production.$BOARD.json" \
    >"$EVIDENCE/restore-drill/stats.production.$BOARD.normalized.json"
  jq 'del(.data.generated_at)' \
    "$EVIDENCE/restore-drill/stats.restore.$BOARD.json" \
    >"$EVIDENCE/restore-drill/stats.restore.$BOARD.normalized.json"
  diff -u \
    "$EVIDENCE/restore-drill/stats.production.$BOARD.normalized.json" \
    "$EVIDENCE/restore-drill/stats.restore.$BOARD.normalized.json" \
    >"$EVIDENCE/restore-drill/stats.$BOARD.diff"
done <"$EVIDENCE/restore-drill/boards.tsv"

DB_INSTANCE_ID="$(
  jq -er '.data.database_instance_id' \
    "$EVIDENCE/preflight/maintenance-status.before-stop.json"
)"
jq -e --arg expected "$DB_INSTANCE_ID" \
  '.data.database_instance_id == $expected' \
  "$EVIDENCE/restore-drill/maintenance-status.json" \
  >"$EVIDENCE/restore-drill/database-instance-id.ok.txt"
assert_writers_stopped
~~~

任何 identity/count/doctor/hash mismatch 都是 hard stop；restore drill 不得修改生产 outbox
或 generation。

## 6. compatibility：pre-admissibility 与 final expected 分离

`maintenance status --json` 是 operator-facing authority，但它只公开真实字段：

- active：generation/fingerprint/fence/provider/provider fingerprint/corpus；
- previous：generation/fingerprint/fence/corpus，**不公开 previous provider**；
- building：generation/fingerprint/fence/provider/provider fingerprint/corpus/phase；
- status 没有 active/previous/building `role` 字段。

因此禁止合成 previous provider 或 role。恢复前只做协议、schema、DB identity、字段闭合性和
runtime pre-admissibility；重建后才把 active provider/corpus 与外部批准的 final expected
逐字段 diff。

批准 TSV 必须恰好使用以下八列和四行数据：

```text
store_name	schema_version	active_provider	active_provider_fingerprint	corpus_schema	corpus_fingerprint	embedding_model	embedding_dimensions
```

Tantivy/Oxigraph 的四个 corpus 字段写字面值 `null`；两个 LanceDB store 的 corpus 字段必须
完整非空。下面的函数只读取 status 的真实字段。

~~~bash
cp --no-preserve=mode "$EXPECTED_BINDINGS_SOURCE" \
  "$EVIDENCE/compatibility/approved-active-bindings.source.tsv"
chmod 0600 "$EVIDENCE/compatibility/approved-active-bindings.source.tsv"
awk -F '\t' '
  NR == 1 {
    expected = "store_name\tschema_version\tactive_provider\tactive_provider_fingerprint\tcorpus_schema\tcorpus_fingerprint\tembedding_model\tembedding_dimensions"
    if ($0 != expected) exit 1
    next
  }
  NF != 8 { exit 1 }
  $1 == "" || $2 != "1" || $3 == "" || $4 == "" { exit 1 }
  ($1 == "tantivy_tasks" || $1 == "oxigraph_relations") &&
    ($5 != "null" || $6 != "null" || $7 != "null" || $8 != "null") { exit 1 }
  ($1 == "lancedb_chunks" || $1 == "lancedb_label_atoms") &&
    ($5 == "null" || $6 == "null" || $7 == "null" ||
     $8 !~ /^[1-9][0-9]*$/) { exit 1 }
  END { if (NR != 5) exit 1 }
' "$EVIDENCE/compatibility/approved-active-bindings.source.tsv"
{
  head -n 1 "$EVIDENCE/compatibility/approved-active-bindings.source.tsv"
  tail -n +2 "$EVIDENCE/compatibility/approved-active-bindings.source.tsv" \
    | LC_ALL=C sort -t $'\t' -k1,1
} >"$EVIDENCE/compatibility/expected-active-bindings.tsv"
chmod 0600 "$EVIDENCE/compatibility/expected-active-bindings.tsv"
tail -n +2 "$EVIDENCE/compatibility/expected-active-bindings.tsv" \
  | cut -f1 | sort \
  >"$EVIDENCE/compatibility/expected-store-names.txt"
printf '%s\n' \
  lancedb_chunks lancedb_label_atoms oxigraph_relations tantivy_tasks \
  | diff -u - "$EVIDENCE/compatibility/expected-store-names.txt" \
  >"$EVIDENCE/compatibility/expected-store-names.diff"

write_active_bindings() {
  local status_file="$1" output_file="$2"
  printf '%s\n' \
    $'store_name\tschema_version\tactive_provider\tactive_provider_fingerprint\tcorpus_schema\tcorpus_fingerprint\tembedding_model\tembedding_dimensions' \
    >"$output_file"
  jq -r '
    .data.stores | sort_by(.store_name)[] |
    [.store_name,.schema_version,
     (.active_provider // "null"),
     (.active_provider_fingerprint // "null"),
     (.active_corpus.corpus_schema // "null"),
     (.active_corpus.corpus_fingerprint // "null"),
     (.active_corpus.embedding_model // "null"),
     (.active_corpus.embedding_dimensions // "null")] | @tsv
  ' "$status_file" >>"$output_file"
}

assert_status_shape() {
  local status_file="$1" output_file="$2" expected_db="$3"
  jq -e --arg db "$expected_db" '
    def corpus_complete:
      . != null and
      (.corpus_schema | type) == "string" and (.corpus_schema | length) > 0 and
      (.corpus_fingerprint | type) == "string" and
        (.corpus_fingerprint | length) > 0 and
      (.embedding_model | type) == "string" and (.embedding_model | length) > 0 and
      (.embedding_dimensions | type) == "number" and .embedding_dimensions > 0;
    .data.database_instance_id == $db and .data.protocol_version == 2 and
    (.data.stores | length) == 4 and
    ([.data.stores[].store_name] | sort) ==
      ["lancedb_chunks","lancedb_label_atoms","oxigraph_relations","tantivy_tasks"] and
    all(.data.stores[];
      .database_instance_id == $db and .protocol_version == 2 and
      .schema_version == 1 and .control_plane == "v2" and
      .runtime_availability == "available" and
      (
        if .active_generation == null then
          .active_fingerprint == null and .active_fence_epoch == null and
          .active_provider == null and .active_provider_fingerprint == null and
          .active_corpus == null
        else
          .active_fingerprint != null and .active_fence_epoch != null and
          .active_provider != null and .active_provider_fingerprint != null and
          (if (.store_name | startswith("lancedb_"))
           then (.active_corpus | corpus_complete)
           else .active_corpus == null end)
        end
      ) and
      (
        if .previous_generation == null then
          .previous_fingerprint == null and .previous_fence_epoch == null and
          .previous_corpus == null
        else
          .previous_fingerprint != null and .previous_fence_epoch != null and
          (if (.store_name | startswith("lancedb_"))
           then (.previous_corpus | corpus_complete)
           else .previous_corpus == null end)
        end
      ) and
      (
        if .building_generation == null then
          .building_fingerprint == null and .building_fence_epoch == null and
          .building_provider == null and .building_provider_fingerprint == null and
          .building_corpus == null and .building_phase == null
        else
          .building_fence_epoch != null and .building_provider != null and
          .building_provider_fingerprint != null and .building_phase != null and
          (if (.store_name | startswith("lancedb_"))
           then (.building_corpus | corpus_complete)
           else .building_corpus == null end)
        end
      )
    )
  ' "$status_file" >"$output_file"
}

assert_final_store_state() {
  local phase="$1" status_file="$2" actual diff_file
  actual="$EVIDENCE/compatibility/$phase.active-bindings.tsv"
  diff_file="$EVIDENCE/compatibility/$phase.active-bindings.diff"
  assert_status_shape "$status_file" \
    "$EVIDENCE/compatibility/$phase.status-shape.ok.txt" "$DB_INSTANCE_ID"
  jq -e '
    (.data.stores | length) == 4 and
    all(.data.stores[];
      .lifecycle_status == "ready" and
      .runtime_availability == "available" and
      .fallback_reason == null and .last_error == null and
      .pending == 0 and .running == 0 and .failed == 0 and .legacy_done == 0 and
      .active_generation != null and .active_fingerprint != null and
      .active_fence_epoch != null and
      .building_generation == null and .building_fingerprint == null and
      .building_fence_epoch == null and .building_provider == null and
      .building_provider_fingerprint == null and .building_corpus == null and
      .building_phase == null
    )
  ' "$status_file" \
    >"$EVIDENCE/compatibility/$phase.stores-ready.ok.txt"
  write_active_bindings "$status_file" "$actual"
  diff -u "$EVIDENCE/compatibility/expected-active-bindings.tsv" "$actual" \
    >"$diff_file"
}

bounded_kanban --db "$DB" --json maintenance status \
  | tee "$EVIDENCE/compatibility/maintenance-status.pre-admissibility.json"
DB_INSTANCE_ID="$(
  jq -er '.data.database_instance_id' \
    "$EVIDENCE/compatibility/maintenance-status.pre-admissibility.json"
)"
assert_status_shape \
  "$EVIDENCE/compatibility/maintenance-status.pre-admissibility.json" \
  "$EVIDENCE/compatibility/pre-admissibility.status-shape.ok.txt" \
  "$DB_INSTANCE_ID"
bounded_sqlite -readonly "$DB" 'PRAGMA user_version;' \
  | tee "$EVIDENCE/compatibility/sqlite.user_version.txt"
test "$(tr -d '[:space:]' <"$EVIDENCE/compatibility/sqlite.user_version.txt")" = 30
assert_writers_stopped
~~~

## 7. 严格 T/O/label-atoms/chunks dry-run

顺序固定为：

1. `tantivy_tasks`
2. `oxigraph_relations`
3. `lancedb_label_atoms`
4. `lancedb_chunks`

每个 store 都先读 status 决定是 fresh 还是 resume。存在 `building_generation` 时只能对同一
store 使用 `--resume`；不存在时禁止 `--resume`。dry-run 不 claim owner/store lease、不
ACK outbox、不创建或发布 generation。每次命令前后都重新断言所有长期 writer 仍停止。

~~~bash
declare -a REBUILD_STORES=(
  tantivy_tasks
  oxigraph_relations
  lancedb_label_atoms
  lancedb_chunks
)
test "$(IFS=,; printf '%s' "${REBUILD_STORES[*]}")" = \
  "tantivy_tasks,oxigraph_relations,lancedb_label_atoms,lancedb_chunks"

for store in "${REBUILD_STORES[@]}"; do
  assert_writers_stopped
  bounded_kanban --db "$DB" --json maintenance status \
    >"$EVIDENCE/compatibility/status.before-dry-run.$store.json"
  building="$(
    jq -r --arg store "$store" \
      '.data.stores[] | select(.store_name == $store) | .building_generation // "null"' \
      "$EVIDENCE/compatibility/status.before-dry-run.$store.json"
  )"
  if test "$building" = null; then
    mode=fresh
    expected_action=dry_run_rebuild
    rebuild_args=(
      bounded_kanban --db "$DB" --actor production-recovery --json
      maintenance rebuild "$store" --dry-run
    )
  else
    mode=resume
    expected_action=dry_run_resume
    rebuild_args=(
      bounded_kanban --db "$DB" --actor production-recovery --json
      maintenance rebuild "$store" --resume --dry-run
    )
  fi
  printf '%s\n' "$mode" >"$EVIDENCE/compatibility/rebuild-mode.$store.txt"
  "${rebuild_args[@]}" \
    | tee "$EVIDENCE/compatibility/dry-run.$store.json"
  jq -e --arg store "$store" --arg action "$expected_action" '
    .data.stores | length == 1 and
    .[0].store_name == $store and
    .[0].result.status == "succeeded" and
    .[0].result.action == $action and
    .[0].result.processed == 0
  ' "$EVIDENCE/compatibility/dry-run.$store.json" \
    >"$EVIDENCE/compatibility/dry-run.$store.ok.txt"
  assert_writers_stopped
done
~~~

没有公开 previous-generation republish 方法。没有 unfinished building generation 时，
合法路径只有新的 fresh rebuild 或保持只读并 escalation；不能手工改 building/previous 字段、
覆盖 marker 或改水位。

## 8. 同 cohort 部署和严格串行 rebuild

按站点批准的部署原语部署 `$COHORT`，但不要启动任何 unit。部署动作完成后，必须先运行
同一个 machine comparison，再证明 maintenance unit 配置的 helper 精确绑定。任一 mismatch
都在 canonical canary mutation 之前 hard stop。

~~~bash
assert_writers_stopped
assert_release_binding post-deploy
assert_configured_unit_binding
assert_writers_stopped
~~~

按 dry-run 保存的 mode，继续以 T/O/label-atoms/chunks 顺序串行 rebuild。每步只允许一个
一次性 owner，命令结束后 singleton owner 必须释放，长期 writer 必须仍为 inactive。

~~~bash
for store in "${REBUILD_STORES[@]}"; do
  assert_writers_stopped
  mode="$(tr -d '\n' <"$EVIDENCE/compatibility/rebuild-mode.$store.txt")"
  case "$mode" in
    fresh)
      rebuild_args=(
        bounded_kanban --db "$DB" --actor production-recovery --json
        maintenance rebuild "$store"
      )
      ;;
    resume)
      rebuild_args=(
        bounded_kanban --db "$DB" --actor production-recovery --json
        maintenance rebuild "$store" --resume
      )
      ;;
    *)
      printf 'invalid saved rebuild mode for %s: %s\n' "$store" "$mode" >&2
      exit 1
      ;;
  esac
  "${rebuild_args[@]}" \
    | tee "$EVIDENCE/compatibility/rebuild.$store.json"
  jq -e --arg store "$store" '
    .data.stores | length == 1 and
    .[0].store_name == $store and
    .[0].result.status == "succeeded"
  ' "$EVIDENCE/compatibility/rebuild.$store.json" \
    >"$EVIDENCE/compatibility/rebuild.$store.ok.txt"
  bounded_kanban --db "$DB" --json maintenance status \
    >"$EVIDENCE/compatibility/status.after.$store.json"
  bounded_kanban --db "$DB" --json outbox list --limit 1000 \
    >"$EVIDENCE/compatibility/outbox.after.$store.json"
  wait_for_lease_expiry
  assert_writers_stopped
done

bounded_kanban --db "$DB" --json maintenance status \
  | tee "$EVIDENCE/compatibility/maintenance-status.after-rebuild.json"
assert_final_store_state after-rebuild \
  "$EVIDENCE/compatibility/maintenance-status.after-rebuild.json"
assert_writers_stopped
~~~

这一步的 empty diff 只比较真实 active provider/corpus 字段。`previous_generation` 仍须通过
status shape 闭合性检查并保留；它不被伪造为 final expected，也不会被清理。

## 9. 启动 continuous owner 后建立 9×4 canary

### 9.1 启动、poll owner 和证明实际 helper 绑定

这是 rebuild 后第一次启动长期 owner，而且只启动 `MAINTENANCE_UNIT`。其它 writer unit
继续要求 `inactive`。在任何 canary mutation 前，poll 必须证明新 MainPID、active
singleton lease、fresh heartbeat、同 cohort build identity、精确四项 capability，以及
四个 store 都由当前 runtime 提供。`hard_stop` 和 `ERR` trap 必须在第一次
`systemctl start` 之前安装；此 trap 从 owner readiness 开始，连续覆盖 helper 检查、
全部 canary、SLA、restart、post-restart mutation、最终 doctor/binding/matrix/delivery
以及 cleanup 后的重新验收，中间不得卸载。`hard_stop` 首先永久卸载当前 shell 的
`ERR` trap，避免递归；并以 `mkdir "$EVIDENCE/sla/.hard-stop-active"` 作为跨
parent/subshell 的一次性闸门：成功创建 marker 的调用执行 cleanup，已存在 marker
的重入只向 stderr 报告并以非零退出，不能再次 stop、signal 或覆盖 result；marker 创建
失败的调用进入 fail-closed cleanup 路径。随后停止全部
unit，并只对原始捕获 identity 仍匹配的非 systemd writer 执行 TERM → timeout → KILL。
若 marker 创建本身失败，不得提前退出：记录 `marker_state=unavailable`、stderr fallback
和 evidence failure 后仍执行全部 bounded stop/probe/idle cleanup；同一 shell 的
初始化阶段已预先打开并持有 `HARD_STOP_GUARD_FD` 的 `flock`；child 通过
`BASHPID != HARD_STOP_OWNER_BASHPID` 只退出并把 ERR 交还 parent，不能重复 cleanup。
`HARD_STOP_ACTIVE` 导出变量仍抑制同 shell 重入，最终以非零关闭。
identity drift 永远只记录、不 signal。最后必须精确证明全部 writer inactive/absent、数据库无
holder、singleton 与四项 store lease exact idle；任一单个 stop、probe 或证据命令 timeout
都累计失败，但仍继续其余 target 和 cleanup。每一次 truncate、append、capture 都必须
检查写入返回值；失败时保留已有证据、设置 `hard_stop_evidence_failed=1` 并向 stderr
输出 fallback，最终 `hard-stop.result.txt` 以 `evidence_write_failed=1` 且非零退出关闭。
status/doctor/outbox 证据采集结束后，必须紧邻 result 再次执行 bounded
writer、database-holder 和 exact-idle 三项断言，封闭诊断窗口内 writer 重启的竞态；
任一清理或证据命令失败仍保留已有证据并以失败关闭。

~~~bash
hard_stop_non_systemd_writers() {
  local evidence_file="$1" index pid expected captured rc pid_state
  local probe_rc identity_rc deadline kill_deadline failed=0 timeout_rc=0
  local signal_result
  if ! : >"$evidence_file"; then
    hard_stop_write_failed "$evidence_file"
    return 1
  fi

  for index in "${!WRITER_PIDS[@]}"; do
    pid="${WRITER_PIDS[$index]}"
    expected="${WRITER_PROCESS_EXES[$index]}"
    if pid_state="$(root_pid_state "$pid")"; then
      probe_rc=0
    else
      probe_rc=$?
      pid_state=error
    fi
    case "$pid_state" in
      absent)
        printf 'pid=%s exe=%s state=absent signal=none\n' \
          "$pid" "$expected" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
        continue
        ;;
      error)
        printf 'pid=%s exe=%s state=error signal=none probe_rc=%s probe_rc_class=%s\n' \
          "$pid" "$expected" "$probe_rc" \
          "$(external_rc_class "$probe_rc")" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
        case "$probe_rc" in
          124|137) test "$timeout_rc" -ne 0 || timeout_rc="$probe_rc" ;;
        esac
        failed=1
        continue
        ;;
      alive) ;;
      *)
        printf 'pid=%s exe=%s state=error signal=none\n' \
          "$pid" "$expected" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
        failed=1
        continue
        ;;
    esac
    captured="${WRITER_START_TIMES[$pid]-}"
    if test -z "$captured"; then
      printf 'pid=%s exe=%s state=alive identity=uncaptured signal=none\n' \
        "$pid" "$expected" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
      failed=1
      continue
    fi
    if assert_same_process_identity "$pid" "$expected" "$captured"; then
      :
    else
      identity_rc=$?
      printf 'pid=%s exe=%s start_time=%s state=alive identity=drift-or-error signal=none rc=%s rc_class=%s\n' \
        "$pid" "$expected" "$captured" "$identity_rc" \
        "$(external_rc_class "$identity_rc")" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
      case "$identity_rc" in
        124|137) test "$timeout_rc" -ne 0 || timeout_rc="$identity_rc" ;;
      esac
      failed=1
      continue
    fi
    if signal_result="$(signal_same_process TERM "$pid" "$expected" "$captured")"; then
      case "$signal_result" in
        alive-signaled)
          printf 'pid=%s exe=%s start_time=%s identity=match signal=TERM\n' \
            "$pid" "$expected" "$captured" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
          ;;
        absent-no-signal)
          printf 'pid=%s exe=%s state=absent signal=none\n' \
            "$pid" "$expected" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
          ;;
        *)
          printf 'pid=%s exe=%s start_time=%s identity=unverified signal=none result=%s\n' \
            "$pid" "$expected" "$captured" "$signal_result" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
          failed=1
          ;;
      esac
    else
      rc=$?
      printf 'pid=%s exe=%s start_time=%s identity=unverified signal=none rc=%s rc_class=%s\n' \
        "$pid" "$expected" "$captured" "$rc" \
        "$(external_rc_class "$rc")" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
      case "$rc" in
        124|137) test "$timeout_rc" -ne 0 || timeout_rc="$rc" ;;
      esac
      failed=1
    fi
  done

  deadline=$(( $(date +%s) + 30 ))
  for index in "${!WRITER_PIDS[@]}"; do
    pid="${WRITER_PIDS[$index]}"
    expected="${WRITER_PROCESS_EXES[$index]}"
    captured="${WRITER_START_TIMES[$pid]-}"
    while :; do
      if pid_state="$(root_pid_state "$pid")"; then
        probe_rc=0
      else
        probe_rc=$?
        printf 'pid=%s exe=%s state=error phase=term-wait signal=none probe_rc=%s probe_rc_class=%s\n' \
          "$pid" "$expected" "$probe_rc" \
          "$(external_rc_class "$probe_rc")" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
        case "$probe_rc" in
          124|137) test "$timeout_rc" -ne 0 || timeout_rc="$probe_rc" ;;
        esac
        failed=1
        break
      fi
      case "$pid_state" in
        absent) break ;;
        alive) ;;
        *)
          printf 'pid=%s exe=%s state=error phase=term-wait signal=none\n' \
            "$pid" "$expected" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
          failed=1
          break
          ;;
      esac
      if test -z "$captured"; then
        failed=1
        break
      fi
      if assert_same_process_identity "$pid" "$expected" "$captured"; then
        :
      else
        identity_rc=$?
        printf 'pid=%s exe=%s start_time=%s state=alive identity=drift-or-error signal=none rc=%s rc_class=%s\n' \
          "$pid" "$expected" "$captured" "$identity_rc" \
            "$(external_rc_class "$identity_rc")" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
        case "$identity_rc" in
          124|137) test "$timeout_rc" -ne 0 || timeout_rc="$identity_rc" ;;
        esac
        failed=1
        break
      fi
      if test "$(date +%s)" -ge "$deadline"; then
        if signal_result="$(signal_same_process KILL "$pid" "$expected" "$captured")"; then
          case "$signal_result" in
            alive-signaled)
              printf 'pid=%s exe=%s start_time=%s identity=match signal=KILL\n' \
                "$pid" "$expected" "$captured" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
              ;;
            absent-no-signal)
              printf 'pid=%s exe=%s state=absent signal=none\n' \
                "$pid" "$expected" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
              ;;
            *)
              printf 'pid=%s exe=%s start_time=%s identity=unverified signal=none result=%s\n' \
                "$pid" "$expected" "$captured" "$signal_result" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
              failed=1
              ;;
          esac
        else
          rc=$?
          printf 'pid=%s exe=%s start_time=%s identity=unverified signal=none rc=%s rc_class=%s\n' \
            "$pid" "$expected" "$captured" "$rc" \
            "$(external_rc_class "$rc")" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
          case "$rc" in
            124|137) test "$timeout_rc" -ne 0 || timeout_rc="$rc" ;;
          esac
          failed=1
        fi
        break
      fi
      if ! sleep 1; then
        failed=1
        break
      fi
    done
  done

  kill_deadline=$(( $(date +%s) + 10 ))
  for index in "${!WRITER_PIDS[@]}"; do
    pid="${WRITER_PIDS[$index]}"
    expected="${WRITER_PROCESS_EXES[$index]}"
    captured="${WRITER_START_TIMES[$pid]-}"
    while :; do
      if pid_state="$(root_pid_state "$pid")"; then
        probe_rc=0
      else
        probe_rc=$?
        printf 'pid=%s exe=%s state=error phase=kill-wait signal=none probe_rc=%s probe_rc_class=%s\n' \
          "$pid" "$expected" "$probe_rc" \
          "$(external_rc_class "$probe_rc")" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
        case "$probe_rc" in
          124|137) test "$timeout_rc" -ne 0 || timeout_rc="$probe_rc" ;;
        esac
        failed=1
        break
      fi
      case "$pid_state" in
        absent) break ;;
        alive) ;;
        *)
          printf 'pid=%s exe=%s state=error phase=kill-wait signal=none\n' \
            "$pid" "$expected" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
          failed=1
          break
          ;;
      esac
      if test -z "$captured"; then
        failed=1
        break
      fi
      if assert_same_process_identity "$pid" "$expected" "$captured"; then
        :
      else
        identity_rc=$?
        printf 'pid=%s exe=%s start_time=%s state=alive identity=drift-or-error signal=none rc=%s rc_class=%s\n' \
          "$pid" "$expected" "$captured" "$identity_rc" \
            "$(external_rc_class "$identity_rc")" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
        case "$identity_rc" in
          124|137) test "$timeout_rc" -ne 0 || timeout_rc="$identity_rc" ;;
        esac
        failed=1
        break
      fi
      if test "$(date +%s)" -ge "$kill_deadline"; then
        failed=1
        break
      fi
      if ! sleep 1; then
        failed=1
        break
      fi
    done
    if pid_state="$(root_pid_state "$pid")"; then
      probe_rc=0
    else
      probe_rc=$?
      pid_state=error
    fi
    case "$pid_state" in
      absent)
        printf 'pid=%s exe=%s state=absent final=stopped\n' \
          "$pid" "$expected" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
        ;;
      alive|error)
        printf 'pid=%s exe=%s state=%s final=failed probe_rc=%s probe_rc_class=%s\n' \
          "$pid" "$expected" "$pid_state" "$probe_rc" \
          "$(external_rc_class "$probe_rc")" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
        case "$probe_rc" in
          124|137) test "$timeout_rc" -ne 0 || timeout_rc="$probe_rc" ;;
        esac
        failed=1
        ;;
      *)
        printf 'pid=%s exe=%s state=error final=failed\n' \
          "$pid" "$expected" >>"$evidence_file" || hard_stop_write_failed "$evidence_file"
        failed=1
        ;;
    esac
  done
  if test "$timeout_rc" -ne 0; then
    return "$timeout_rc"
  fi
  return "$failed"
}

hard_stop_evidence_failed=0

hard_stop_record() {
  local evidence_file="$1"; shift
  if ! printf "$@" >>"$evidence_file"; then
    hard_stop_evidence_failed=1
    printf 'hard-stop evidence write failed: %s\n' "$evidence_file" >&2
    return 1
  fi
}

hard_stop_capture() {
  local evidence_file="$1"; shift
  if "$@" >"$evidence_file"; then
    return 0
  fi
  local rc=$?
  hard_stop_evidence_failed=1
  printf 'hard-stop evidence capture failed: %s rc=%s\n' "$evidence_file" "$rc" >&2
  return "$rc"
}

hard_stop_write_failed() {
  hard_stop_evidence_failed=1
  printf 'hard-stop evidence append failed: %s\n' "$1" >&2
  return 0
}

hard_stop() {
  local reason="$1" unit state rc failed=0 marker marker_state=unavailable
  trap - ERR
  set +E
  set +e
  if test "${HARD_STOP_OWNER_BASHPID:-$BASHPID}" != "$BASHPID"; then
    printf 'hard-stop delegated from child BASHPID=%s to owner BASHPID=%s\n' \
      "$BASHPID" "${HARD_STOP_OWNER_BASHPID:-unknown}" >&2
    exit 1
  fi
  if test "${HARD_STOP_ACTIVE:-0}" -eq 1; then
    printf 'hard-stop already active; suppressing re-entry\n' >&2
    exit 1
  fi
  export HARD_STOP_ACTIVE=1
  marker="$EVIDENCE/sla/.hard-stop-active"
  if mkdir "$marker" 2>/dev/null; then
    marker_state=owned
  elif test -d "$marker"; then
    printf 'hard-stop already active; suppressing re-entry\n' >&2
    exit 1
  else
    hard_stop_evidence_failed=1
    printf 'hard-stop re-entry guard could not be created; continuing fail-closed cleanup: %s\n' "$marker" >&2
    failed=1
  fi
  hard_stop_record "$EVIDENCE/sla/hard-stop.reason.txt" \
    'reason=%s epoch=%s\n' "$reason" "$(date +%s)" || failed=1
  if ! : >"$EVIDENCE/sla/hard-stop.units.txt"; then
    hard_stop_evidence_failed=1
    printf 'hard-stop evidence truncate failed: %s\n' "$EVIDENCE/sla/hard-stop.units.txt" >&2
    failed=1
  fi
  if ! : >"$EVIDENCE/sla/hard-stop.commands.txt"; then
    hard_stop_evidence_failed=1
    printf 'hard-stop evidence truncate failed: %s\n' "$EVIDENCE/sla/hard-stop.commands.txt" >&2
    failed=1
  fi
  for unit in "${WRITER_UNITS[@]}"; do
    if bounded_sudo systemctl stop "$unit"; then
      rc=0
    else
      rc=$?
    fi
    printf 'unit=%s action=stop rc=%s rc_class=%s\n' \
      "$unit" "$rc" "$(external_rc_class "$rc")" \
      >>"$EVIDENCE/sla/hard-stop.units.txt" || hard_stop_write_failed "$EVIDENCE/sla/hard-stop.units.txt"
    if test "$rc" -ne 0; then
      failed=1
    fi
    if bounded_sudo systemctl reset-failed "$unit"; then
      rc=0
    else
      rc=$?
    fi
    printf 'unit=%s action=reset-failed rc=%s rc_class=%s\n' \
      "$unit" "$rc" "$(external_rc_class "$rc")" \
      >>"$EVIDENCE/sla/hard-stop.units.txt" || hard_stop_write_failed "$EVIDENCE/sla/hard-stop.units.txt"
    if test "$rc" -ne 0; then
      failed=1
    fi
    if state="$(bounded_sudo systemctl is-active "$unit" 2>&1)"; then
      rc=0
    else
      rc=$?
    fi
    printf 'unit=%s state=%s is-active-rc=%s rc_class=%s\n' \
      "$unit" "$state" "$rc" "$(external_rc_class "$rc")" \
      >>"$EVIDENCE/sla/hard-stop.units.txt" || hard_stop_write_failed "$EVIDENCE/sla/hard-stop.units.txt"
    if test "$state" != inactive || test "$rc" -ne 3; then
      failed=1
    fi
  done
  if hard_stop_non_systemd_writers \
    "$EVIDENCE/sla/hard-stop.non-systemd-writers.txt"; then
    rc=0
  else
    rc=$?
  fi
  printf 'target=non-systemd-writers rc=%s rc_class=%s\n' \
    "$rc" "$(external_rc_class "$rc")" \
    >>"$EVIDENCE/sla/hard-stop.commands.txt" || hard_stop_write_failed "$EVIDENCE/sla/hard-stop.commands.txt"
  if test "$rc" -ne 0; then
    failed=1
  fi
  if assert_writers_stopped \
    "$EVIDENCE/sla/hard-stop.writers.assert-stopped.txt"; then
    rc=0
  else
    rc=$?
  fi
  printf 'target=writers-initial rc=%s rc_class=%s\n' \
    "$rc" "$(external_rc_class "$rc")" \
    >>"$EVIDENCE/sla/hard-stop.commands.txt" || hard_stop_write_failed "$EVIDENCE/sla/hard-stop.commands.txt"
  if test "$rc" -ne 0; then
    failed=1
  fi
  if assert_no_database_holders \
    "$EVIDENCE/sla/hard-stop.database-holders.txt"; then
    rc=0
  else
    rc=$?
  fi
  printf 'target=database-holders-initial rc=%s rc_class=%s\n' \
    "$rc" "$(external_rc_class "$rc")" \
    >>"$EVIDENCE/sla/hard-stop.commands.txt" || hard_stop_write_failed "$EVIDENCE/sla/hard-stop.commands.txt"
  if test "$rc" -ne 0; then
    failed=1
  fi
  if wait_for_exact_maintenance_release "$EVIDENCE/sla" hard-stop; then
    rc=0
  else
    rc=$?
  fi
  printf 'target=maintenance-exact-idle-initial rc=%s rc_class=%s\n' \
    "$rc" "$(external_rc_class "$rc")" \
    >>"$EVIDENCE/sla/hard-stop.commands.txt" || hard_stop_write_failed "$EVIDENCE/sla/hard-stop.commands.txt"
  if test "$rc" -ne 0; then
    failed=1
  fi
  if hard_stop_capture "$EVIDENCE/sla/hard-stop.maintenance-status.json" \
    bounded_kanban --db "$DB" --json maintenance status; then
    rc=0
  else
    rc=$?
  fi
  printf 'target=maintenance-status rc=%s rc_class=%s\n' \
    "$rc" "$(external_rc_class "$rc")" \
    >>"$EVIDENCE/sla/hard-stop.commands.txt" || hard_stop_write_failed "$EVIDENCE/sla/hard-stop.commands.txt"
  if test "$rc" -ne 0; then
    failed=1
  fi
  if hard_stop_capture "$EVIDENCE/sla/hard-stop.doctor.json" \
    bounded_kanban --db "$DB" --json doctor; then
    rc=0
  else
    rc=$?
  fi
  printf 'target=doctor rc=%s rc_class=%s\n' \
    "$rc" "$(external_rc_class "$rc")" \
    >>"$EVIDENCE/sla/hard-stop.commands.txt" || hard_stop_write_failed "$EVIDENCE/sla/hard-stop.commands.txt"
  if test "$rc" -ne 0; then
    failed=1
  fi
  if hard_stop_capture "$EVIDENCE/sla/hard-stop.outbox.json" \
    bounded_kanban --db "$DB" --json outbox list --limit 1000; then
    rc=0
  else
    rc=$?
  fi
  printf 'target=outbox rc=%s rc_class=%s\n' \
    "$rc" "$(external_rc_class "$rc")" \
    >>"$EVIDENCE/sla/hard-stop.commands.txt" || hard_stop_write_failed "$EVIDENCE/sla/hard-stop.commands.txt"
  if test "$rc" -ne 0; then
    failed=1
  fi
  if assert_writers_stopped \
    "$EVIDENCE/sla/hard-stop.final.writers.assert-stopped.txt"; then
    rc=0
  else
    rc=$?
  fi
  printf 'target=writers-final rc=%s rc_class=%s\n' \
    "$rc" "$(external_rc_class "$rc")" \
    >>"$EVIDENCE/sla/hard-stop.commands.txt" || hard_stop_write_failed "$EVIDENCE/sla/hard-stop.commands.txt"
  if test "$rc" -ne 0; then
    failed=1
  fi
  if assert_no_database_holders \
    "$EVIDENCE/sla/hard-stop.final.database-holders.txt"; then
    rc=0
  else
    rc=$?
  fi
  printf 'target=database-holders-final rc=%s rc_class=%s\n' \
    "$rc" "$(external_rc_class "$rc")" \
    >>"$EVIDENCE/sla/hard-stop.commands.txt" || hard_stop_write_failed "$EVIDENCE/sla/hard-stop.commands.txt"
  if test "$rc" -ne 0; then
    failed=1
  fi
  if wait_for_exact_maintenance_release "$EVIDENCE/sla" hard-stop-final; then
    rc=0
  else
    rc=$?
  fi
  printf 'target=maintenance-exact-idle-final rc=%s rc_class=%s\n' \
    "$rc" "$(external_rc_class "$rc")" \
    >>"$EVIDENCE/sla/hard-stop.commands.txt" || hard_stop_write_failed "$EVIDENCE/sla/hard-stop.commands.txt"
  if test "$rc" -ne 0; then
    failed=1
  fi
  if test "$hard_stop_evidence_failed" -ne 0; then
    failed=1
  fi
  if ! hard_stop_record "$EVIDENCE/sla/hard-stop.result.txt" \
    'cleanup_complete=%s exit=1 evidence_write_failed=%s marker_state=%s\n' \
    "$(( failed == 0 ))" "$hard_stop_evidence_failed" "$marker_state"; then
    printf 'hard-stop result evidence unavailable; cleanup_complete=0 exit=1\n' >&2
  fi
  exit 1
}

trap 'hard_stop "fatal-command-error line=$LINENO"' ERR

assert_non_owner_writers_stopped() {
  local unit state rc pid pid_state probe_rc
  for unit in "${WRITER_UNITS[@]}"; do
    if test "$unit" = "$MAINTENANCE_UNIT"; then
      continue
    fi
    if state="$(bounded_sudo systemctl is-active "$unit" 2>&1)"; then
      rc=0
    else
      rc=$?
    fi
    if test "$state" != inactive || test "$rc" -ne 3; then
      case "$rc" in
        124|137) return "$rc" ;;
        *) return 1 ;;
      esac
    fi
  done
  for pid in "${WRITER_PIDS[@]}"; do
    if pid_state="$(root_pid_state "$pid")"; then
      probe_rc=0
    else
      probe_rc=$?
      return "$probe_rc"
    fi
    test "$pid_state" = absent
  done
}

assert_live_unit_binding() {
  local pid="$1" output_file="$2"
  [[ "$pid" =~ ^[1-9][0-9]*$ ]]
  bounded_sudo python3 - "$pid" "$KANBAN_BIN" \
    "$KANBAN_VECTOR_HELPER" "$KANBAN_GRAPH_HELPER" \
    >"$output_file" <<'PY'
import json
import os
import pathlib
import sys

pid, expected_kanban, expected_vector, expected_graph = sys.argv[1:]
live_executable = pathlib.Path(f"/proc/{pid}/exe").resolve(strict=True)
if not os.path.samefile(live_executable, pathlib.Path(expected_kanban)):
    raise SystemExit("live maintenance process executable is not deployed kanban")
raw = pathlib.Path(f"/proc/{pid}/environ").read_bytes()
environment = {}
for item in raw.split(b"\0"):
    if b"=" in item:
        key, value = item.split(b"=", 1)
        environment[key.decode(errors="strict")] = value.decode(errors="strict")
selected = {
    "KANBAN_VECTOR_HELPER": environment.get("KANBAN_VECTOR_HELPER"),
    "KANBAN_GRAPH_HELPER": environment.get("KANBAN_GRAPH_HELPER"),
}
expected = {
    "KANBAN_VECTOR_HELPER": expected_vector,
    "KANBAN_GRAPH_HELPER": expected_graph,
}
if selected != expected:
    raise SystemExit("live maintenance process helper binding mismatch")
print(json.dumps({
    "main_executable": str(live_executable),
    "selected_environment": selected,
    "exact_match": True,
}, sort_keys=True))
PY
}

assert_non_owner_writers_stopped
OWNER_START_MS=$(( $(date +%s) * 1000 ))
bounded_sudo systemctl start "$MAINTENANCE_UNIT"
OWNER_START_DEADLINE=$(( $(date +%s) + OWNER_START_TIMEOUT_SEC ))
while :; do
  OWNER_PID="$(
    bounded_sudo systemctl show "$MAINTENANCE_UNIT" \
      --property=MainPID --value
  )"
  bounded_kanban --db "$DB" --json maintenance status \
    >"$EVIDENCE/canaries/owner-start.status.current.json"
  now_ms=$(( $(date +%s) * 1000 ))
  if [[ "$OWNER_PID" =~ ^[1-9][0-9]*$ ]] &&
    jq -e --arg build_id \
      "$(jq -r '.build_id' "$COHORT/source-provenance.json")" \
      --argjson started "$OWNER_START_MS" --argjson now "$now_ms" '
      .data.maintenance_owner.active == true and
      .data.maintenance_owner.mode == "continuous" and
      .data.maintenance_owner.build_identity == $build_id and
      (.data.maintenance_owner.last_heartbeat_at >= $started) and
      (.data.maintenance_owner.lease_expires_at > $now) and
      (.data.maintenance_owner.capabilities | sort) ==
        ["lancedb_chunks","lancedb_label_atoms",
         "oxigraph_relations","tantivy_tasks"] and
      (.data.stores | length) == 4 and
      ([.data.stores[].store_name] | sort) ==
        ["lancedb_chunks","lancedb_label_atoms",
         "oxigraph_relations","tantivy_tasks"] and
      all(.data.stores[]; .runtime_availability == "available")
    ' "$EVIDENCE/canaries/owner-start.status.current.json" >/dev/null; then
    break
  fi
  test "$(date +%s)" -lt "$OWNER_START_DEADLINE"
  sleep 1
done
cp --no-preserve=mode "$EVIDENCE/canaries/owner-start.status.current.json" \
  "$EVIDENCE/canaries/owner-start.status.ready.json"
printf '%s\n' "$OWNER_PID" >"$EVIDENCE/canaries/owner-start.main-pid.txt"
bounded_sudo systemctl is-active --quiet "$MAINTENANCE_UNIT"
assert_non_owner_writers_stopped
assert_live_unit_binding "$OWNER_PID" \
  "$EVIDENCE/canaries/owner-start.live-helper-environment.redacted.json"
~~~

### 9.2 建立 task 与 label-semantics 两类独立 canary

Task canary 只有三路 delivery：Tantivy、Oxigraph、LanceDB chunks。Label atoms 使用独立
label-semantics mutation。每个 board 在 mutation 前后使用 `sqlite3 -readonly -json`
保存精确 event/outbox/delivery ID；这里只读查询 canonical control data，不写数据库。

~~~bash
bounded_kanban --db "$DB" --json board list --include-archived \
  | tee "$EVIDENCE/canaries/boards.json"
jq -e '[.data[] | select(.archived_at == null)] | length == 9' \
  "$EVIDENCE/canaries/boards.json" \
  >"$EVIDENCE/canaries/boards.nine.ok.txt"
jq -r '.data[] | select(.archived_at == null) | [.slug,.id] | @tsv' \
  "$EVIDENCE/canaries/boards.json" \
  >"$EVIDENCE/canaries/boards.tsv"
test "$(wc -l <"$EVIDENCE/canaries/boards.tsv")" -eq 9
test "$(cut -f1 "$EVIDENCE/canaries/boards.tsv" | sort -u | wc -l)" -eq 9
test "$(cut -f2 "$EVIDENCE/canaries/boards.tsv" | sort -u | wc -l)" -eq 9

while IFS=$'\t' read -r BOARD BOARD_ID; do
  [[ "$BOARD_ID" =~ ^b_[A-Za-z0-9]+$ ]]
  TASK_TEXT="projv2taskcanary${RECOVERY_ID//[^A-Za-z0-9]/}${BOARD//[^A-Za-z0-9]/}"
  LABEL_NAME="projection-v2-label-canary-$RECOVERY_ID-$BOARD"
  LABEL_TEXT="projv2labelatomcanary${RECOVERY_ID//[^A-Za-z0-9]/}${BOARD//[^A-Za-z0-9]/}"

  bounded_kanban --db "$DB" --board "$BOARD" \
    --actor production-recovery --json \
    task create "$TASK_TEXT" --description "$TASK_TEXT canonical task canary" \
    >"$EVIDENCE/canaries/$BOARD.task-create.json"
  jq -e --arg board_id "$BOARD_ID" '
    (.data.id | type) == "string" and
    (.data.ref | type) == "string" and
    .data.board_id == $board_id and
    (.data.title | type) == "string"
  ' "$EVIDENCE/canaries/$BOARD.task-create.json" \
    >"$EVIDENCE/canaries/$BOARD.task-identity.ok.txt"
  TASK_ID="$(
    jq -er '.data.id' "$EVIDENCE/canaries/$BOARD.task-create.json"
  )"
  [[ "$TASK_ID" =~ ^t_[A-Za-z0-9]+$ ]]

  bounded_sqlite -readonly -json "$DB" "
    SELECT
      e.id AS event_row_id,e.event_id,o.id AS outbox_id,
      d.id AS delivery_id,d.store_name,d.board_id,d.source_event_id,
      d.cursor,d.entity_uri
    FROM task_events e
    JOIN index_outbox o ON o.source_event_id=e.id
    JOIN projection_deliveries d
      ON d.outbox_id=o.id AND d.source_event_id=e.id
    WHERE e.task_id='$TASK_ID'
      AND e.board_id='$BOARD_ID'
      AND e.kind='task.created'
      AND e.actor='production-recovery'
      AND o.entity_uri='kb://task/$TASK_ID'
      AND d.store_name IN (
        'tantivy_tasks','oxigraph_relations','lancedb_chunks'
      )
    ORDER BY d.store_name;
  " >"$EVIDENCE/canaries/$BOARD.task-delivery-ids.json"
  jq -e --arg board_id "$BOARD_ID" --arg task_uri "kb://task/$TASK_ID" '
    length == 3 and
    ([.[].store_name] | sort) ==
      ["lancedb_chunks","oxigraph_relations","tantivy_tasks"] and
    ([.[].event_row_id] | unique | length) == 1 and
    ([.[].event_id] | unique | length) == 1 and
    ([.[].outbox_id] | unique | length) == 3 and
    ([.[].delivery_id] | unique | length) == 3 and
    all(.[]; .board_id == $board_id and .entity_uri == $task_uri and
              .cursor == .outbox_id and .source_event_id == .event_row_id)
  ' "$EVIDENCE/canaries/$BOARD.task-delivery-ids.json" \
    >"$EVIDENCE/canaries/$BOARD.task-delivery-ids.ok.txt"

  bounded_kanban --db "$DB" --board "$BOARD" \
    --actor production-recovery --json \
    label create "$LABEL_NAME" \
    >"$EVIDENCE/canaries/$BOARD.label-create.json"
  jq -e --arg board_id "$BOARD_ID" '
    (.data.id | type) == "string" and
    .data.board_id == $board_id and
    (.data.name | type) == "string"
  ' "$EVIDENCE/canaries/$BOARD.label-create.json" \
    >"$EVIDENCE/canaries/$BOARD.label-identity.ok.txt"

  bounded_sqlite -readonly -json "$DB" \
    'SELECT COALESCE(MAX(id),0) AS max_id FROM index_outbox;' \
    >"$EVIDENCE/canaries/$BOARD.label-outbox-baseline.json"
  LABEL_OUTBOX_BASELINE="$(
    jq -er '.[0].max_id' \
      "$EVIDENCE/canaries/$BOARD.label-outbox-baseline.json"
  )"
  [[ "$LABEL_OUTBOX_BASELINE" =~ ^[0-9]+$ ]]

  bounded_kanban --db "$DB" --board "$BOARD" \
    --actor production-recovery --json \
    label semantics upsert "$LABEL_NAME" --replace \
      --description "$LABEL_TEXT" \
      --positive-example "$LABEL_TEXT" \
      --reason "Projection v2 label-atoms canary $RECOVERY_ID" \
    >"$EVIDENCE/canaries/$BOARD.label-semantics-upsert.json"
  jq -e --arg board_id "$BOARD_ID" --arg text "$LABEL_TEXT" '
    (.data.label_id | type) == "string" and
    .data.board_id == $board_id and
    (.data.label_name | type) == "string" and
    (.data.semantics_hash | type) == "string" and
    .data.description == $text and
    (.data.atoms | length) > 0
  ' "$EVIDENCE/canaries/$BOARD.label-semantics-upsert.json" \
    >"$EVIDENCE/canaries/$BOARD.label-semantics-identity.ok.txt"

  bounded_sqlite -readonly -json "$DB" "
    SELECT
      o.id AS outbox_id,d.id AS delivery_id,d.store_name,d.board_id,
      d.source_event_id,d.cursor,d.entity_uri,o.projection_store,
      o.action,o.payload_json
    FROM index_outbox o
    JOIN projection_deliveries d ON d.outbox_id=o.id
    WHERE o.id > $LABEL_OUTBOX_BASELINE
      AND o.source_event_id IS NULL
      AND o.target='lancedb'
      AND o.projection_store='lancedb_label_atoms'
      AND o.entity_uri='kb://board/$BOARD_ID'
      AND o.action='rebuild'
      AND o.payload_json='{\"scope\":\"board\",\"version\":1}'
      AND d.store_name='lancedb_label_atoms'
      AND d.board_id='$BOARD_ID'
    ORDER BY o.id;
  " >"$EVIDENCE/canaries/$BOARD.label-delivery-id.json"
  jq -e --arg board_id "$BOARD_ID" '
    length == 1 and
    .[0].store_name == "lancedb_label_atoms" and
    .[0].projection_store == "lancedb_label_atoms" and
    .[0].board_id == $board_id and
    .[0].source_event_id == null and
    .[0].cursor == .[0].outbox_id
  ' "$EVIDENCE/canaries/$BOARD.label-delivery-id.json" \
    >"$EVIDENCE/canaries/$BOARD.label-delivery-id.ok.txt"

  bounded_kanban --db "$DB" --board "$BOARD" \
    --actor production-recovery --json events \
    >"$EVIDENCE/canaries/$BOARD.events.after-canary.json"
done <"$EVIDENCE/canaries/boards.tsv"

bounded_kanban --db "$DB" --json outbox list --limit 1000 \
  >"$EVIDENCE/canaries/outbox.after-all-canaries.json"
~~~

### 9.3 可重复的 36-query matrix

搜索使用唯一 canary 文本，而不是 task ref；task-ref 形状按公开契约强制走 SQLite exact
path，不能证明 Tantivy。chunk CLI 输出只有 `chunk.entity_uri`，没有 `board_id`；因此函数
同时保存：

- `board show` 的 requested slug → resolved board ID；
- public CLI chunk query；
- helper 的 `--board-id` preflight guard probe；
- 每个 chunk `entity_uri` 到 canonical `entities.board_id` 的只读映射。

函数总是生成恰好 36 个唯一矩阵行。合法但尚未收敛的空命中写 `FAIL` 并返回 1；命令/JSON
contract 错误返回 2；任一 cross-board hit 返回 3。空结果不会被当作 PASS。

~~~bash
run_query_matrix() {
  local output_dir="$1" matrix="$2"
  local BOARD BOARD_ID TASK_ID TASK_REF TASK_TEXT LABEL_ID LABEL_TEXT
  local search_file graph_file chunks_file labels_file board_file helper_file
  local hit_count target_hits cross_hits resolved pass uri row
  local incomplete=0 cross_board_failure=0
  local -a vector_config_args=()

  install -d -m 0700 "$output_dir"
  if test -n "$VECTOR_CONFIG"; then
    case "$VECTOR_CONFIG" in
      /*) ;;
      *) return 2 ;;
    esac
    vector_config_args=(--vector-config "$VECTOR_CONFIG")
  fi
  printf '%s\n' \
    $'board\tstore\tidentity\trequested_board\tresolved_board_id\thit_count\ttarget_hits\tcross_board_hits\tpass' \
    >"$matrix"

  while IFS=$'\t' read -r BOARD BOARD_ID; do
    TASK_ID="$(jq -er '.data.id' "$EVIDENCE/canaries/$BOARD.task-create.json")" \
      || return 2
    TASK_REF="$(jq -er '.data.ref' "$EVIDENCE/canaries/$BOARD.task-create.json")" \
      || return 2
    TASK_TEXT="$(jq -er '.data.title' "$EVIDENCE/canaries/$BOARD.task-create.json")" \
      || return 2
    LABEL_ID="$(
      jq -er '.data.label_id' \
        "$EVIDENCE/canaries/$BOARD.label-semantics-upsert.json"
    )" || return 2
    LABEL_TEXT="$(
      jq -er '.data.description' \
        "$EVIDENCE/canaries/$BOARD.label-semantics-upsert.json"
    )" || return 2

    board_file="$output_dir/$BOARD.0-board-resolution.json"
    bounded_kanban --db "$DB" --json board show "$BOARD" >"$board_file" \
      || return 2
    jq -e --arg requested "$BOARD" --arg resolved "$BOARD_ID" '
      .data.slug == $requested and .data.id == $resolved
    ' "$board_file" >"$output_dir/$BOARD.0-board-resolution.ok.txt" \
      || return 2
    resolved="$(jq -er '.data.id' "$board_file")" || return 2

    search_file="$output_dir/$BOARD.1-tantivy_tasks.json"
    bounded_kanban --db "$DB" --board "$BOARD" --json \
      search "$TASK_TEXT" --limit 20 >"$search_file" || return 2
    jq -e '
      (.data.hits | type) == "array" and
      (.meta.backend | type) == "string" and
      (.meta.stale | type) == "boolean" and
      (.meta.resolved_board_id | type) == "string"
    ' "$search_file" >/dev/null || return 2
    hit_count="$(jq '.data.hits | length' "$search_file")" || return 2
    target_hits="$(
      jq --arg id "$TASK_ID" --arg ref "$TASK_REF" --arg board_id "$BOARD_ID" '
        [.data.hits[] |
         select(.task.id == $id and .task.ref == $ref and
                .task.board_id == $board_id)] | length
      ' "$search_file"
    )" || return 2
    cross_hits="$(
      jq --arg board_id "$BOARD_ID" \
        '[.data.hits[] | select(.task.board_id != $board_id)] | length' \
        "$search_file"
    )" || return 2
    pass=FAIL
    if test "$hit_count" -gt 0 &&
      test "$target_hits" -eq 1 &&
      test "$cross_hits" -eq 0 &&
      jq -e --arg board_id "$BOARD_ID" '
        .meta.backend == "tantivy" and .meta.stale == false and
        .meta.resolved_board_id == $board_id
      ' "$search_file" >/dev/null; then
      pass=PASS
    else
      incomplete=1
    fi
    test "$cross_hits" -eq 0 || cross_board_failure=1
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
      "$BOARD" tantivy_tasks "$TASK_ID" "$BOARD" "$resolved" \
      "$hit_count" "$target_hits" "$cross_hits" "$pass" >>"$matrix"

    graph_file="$output_dir/$BOARD.2-oxigraph_relations.json"
    bounded_kanban --db "$DB" --board "$BOARD" --json \
      graph neighbors "kb://task/$TASK_ID" \
      --predicate belongs_to_board --limit 20 >"$graph_file" || return 2
    jq -e '(.data | type) == "array"' "$graph_file" >/dev/null || return 2
    hit_count="$(jq '.data | length' "$graph_file")" || return 2
    target_hits="$(
      jq --arg subject "kb://task/$TASK_ID" --arg object "kb://board/$BOARD_ID" '
        [.data[] |
         select(.subject_uri == $subject and
                .predicate == "belongs_to_board" and
                .object_uri == $object)] | length
      ' "$graph_file"
    )" || return 2
    cross_hits="$(
      jq --arg subject "kb://task/$TASK_ID" --arg object "kb://board/$BOARD_ID" '
        [.data[] |
         select(.subject_uri != $subject or
                .predicate != "belongs_to_board" or
                .object_uri != $object)] | length
      ' "$graph_file"
    )" || return 2
    pass=FAIL
    if test "$hit_count" -gt 0 &&
      test "$target_hits" -gt 0 &&
      test "$cross_hits" -eq 0; then
      pass=PASS
    else
      incomplete=1
    fi
    test "$cross_hits" -eq 0 || cross_board_failure=1
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
      "$BOARD" oxigraph_relations "$TASK_ID" "$BOARD" "$resolved" \
      "$hit_count" "$target_hits" "$cross_hits" "$pass" >>"$matrix"

    chunks_file="$output_dir/$BOARD.3-lancedb_chunks.json"
    bounded_kanban --db "$DB" --board "$BOARD" --json \
      vector query-chunks "$TASK_TEXT" --limit 20 \
      "${vector_config_args[@]}" >"$chunks_file" || return 2
    jq -e '(.data | type) == "array"' "$chunks_file" >/dev/null || return 2
    helper_file="$output_dir/$BOARD.3-lancedb_chunks.board-guard-helper.json"
    bounded_vector_helper query-chunks \
      --db "$DB" --board "$BOARD" --board-id "$BOARD_ID" \
      --text "$TASK_TEXT" --limit 20 "${vector_config_args[@]}" \
      >"$helper_file" || return 2
    jq -e '
      .protocol == "kanban-derived-helper.v1" and
      ((.payload_json | fromjson | type) == "array")
    ' "$helper_file" >/dev/null || return 2

    : >"$output_dir/$BOARD.3-lancedb_chunks.entity-board-map.tsv"
    while IFS= read -r uri; do
      [[ "$uri" =~ ^kb://task/t_[A-Za-z0-9]+$ ]] || return 2
      row="$(
        bounded_sqlite -readonly -noheader -separator $'\t' "$DB" \
          "SELECT uri,board_id FROM entities WHERE uri='$uri';"
      )" || return 2
      test -n "$row" || return 2
      test "$(printf '%s\n' "$row" | wc -l)" -eq 1 || return 2
      printf '%s\n' "$row" \
        >>"$output_dir/$BOARD.3-lancedb_chunks.entity-board-map.tsv"
    done < <(jq -r '.data[].chunk.entity_uri' "$chunks_file")
    hit_count="$(jq '.data | length' "$chunks_file")" || return 2
    target_hits="$(
      jq --arg uri "kb://task/$TASK_ID" \
        '[.data[] | select(.chunk.entity_uri == $uri)] | length' \
        "$chunks_file"
    )" || return 2
    cross_hits="$(
      awk -F '\t' -v expected="$BOARD_ID" \
        '$2 != expected { count++ } END { print count+0 }' \
        "$output_dir/$BOARD.3-lancedb_chunks.entity-board-map.tsv"
    )" || return 2
    pass=FAIL
    if test "$hit_count" -gt 0 &&
      test "$target_hits" -gt 0 &&
      test "$cross_hits" -eq 0 &&
      jq -e --arg uri "kb://task/$TASK_ID" \
        '(.payload_json | fromjson) as $payload |
         [$payload[] | select(.chunk.entity_uri == $uri)] | length > 0' \
        "$helper_file" >/dev/null; then
      pass=PASS
    else
      incomplete=1
    fi
    test "$cross_hits" -eq 0 || cross_board_failure=1
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
      "$BOARD" lancedb_chunks "$TASK_ID" "$BOARD" "$resolved" \
      "$hit_count" "$target_hits" "$cross_hits" "$pass" >>"$matrix"

    labels_file="$output_dir/$BOARD.4-lancedb_label_atoms.json"
    bounded_kanban --db "$DB" --board "$BOARD" --json \
      vector query-label-atoms "$LABEL_TEXT" \
      --board-id "$BOARD_ID" --limit 20 "${vector_config_args[@]}" \
      >"$labels_file" || return 2
    jq -e '(.data | type) == "array"' "$labels_file" >/dev/null || return 2
    hit_count="$(jq '.data | length' "$labels_file")" || return 2
    target_hits="$(
      jq --arg label_id "$LABEL_ID" --arg board_id "$BOARD_ID" --arg text "$LABEL_TEXT" '
        [.data[] |
         select(.label_id == $label_id and .board_id == $board_id and
                .text == $text and .polarity == "positive" and
                .kind == "positive_example")] | length
      ' "$labels_file"
    )" || return 2
    cross_hits="$(
      jq --arg board_id "$BOARD_ID" \
        '[.data[] | select(.board_id != $board_id)] | length' \
        "$labels_file"
    )" || return 2
    pass=FAIL
    if test "$hit_count" -gt 0 &&
      test "$target_hits" -gt 0 &&
      test "$cross_hits" -eq 0; then
      pass=PASS
    else
      incomplete=1
    fi
    test "$cross_hits" -eq 0 || cross_board_failure=1
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
      "$BOARD" lancedb_label_atoms "$LABEL_ID" "$BOARD" "$resolved" \
      "$hit_count" "$target_hits" "$cross_hits" "$pass" >>"$matrix"
  done <"$EVIDENCE/canaries/boards.tsv"

  test "$(tail -n +2 "$matrix" | wc -l)" -eq 36 || return 2
  test "$(tail -n +2 "$matrix" | cut -f1,2 | sort -u | wc -l)" -eq 36 \
    || return 2
  awk -F '\t' 'NR > 1 && $6 <= 0 { empty++ } END { exit(empty == 0 ? 0 : 1) }' \
    "$matrix" || incomplete=1
  if test "$cross_board_failure" -ne 0; then
    return 3
  fi
  if test "$incomplete" -ne 0; then
    return 1
  fi
  awk -F '\t' 'NR > 1 && ($8 != 0 || $9 != "PASS") { exit 1 }' "$matrix"
}
~~~

## 10. 每 5 秒重跑 36 查询、四-store snapshot 和 delivery proof

### 10.1 exact read-only delivery proof

完成态 delivery 的 `claim_fence_epoch` 按 schema 会被清为 `NULL`，所以不能伪造“ACK fence
字段”。正确证明是把精确 canary event/outbox/delivery ID 联到 `projection_store_state`，
要求：

- task event 恰好三路、label outbox 恰好一路；
- `status='done'` 且 `published_generation=active_generation`；
- `cursor=outbox_id` 且 `cursor <= checkpoint_cursor`；
- `active_fence_epoch` 非空，当前 `fence_epoch >= active_fence_epoch`。

下面每个 poll 都重新执行这条 `sqlite3 -readonly -json` 证明。

~~~bash
run_delivery_proof() {
  local output_dir="$1" BOARD BOARD_ID TASK_ID LABEL_OUTBOX_ID actual expected
  local ready_boards=0
  install -d -m 0700 "$output_dir"

  while IFS=$'\t' read -r BOARD BOARD_ID; do
    TASK_ID="$(jq -er '.data.id' "$EVIDENCE/canaries/$BOARD.task-create.json")" \
      || return 2
    LABEL_OUTBOX_ID="$(
      jq -er '.[0].outbox_id' \
        "$EVIDENCE/canaries/$BOARD.label-delivery-id.json"
    )" || return 2
    [[ "$TASK_ID" =~ ^t_[A-Za-z0-9]+$ ]] || return 2
    [[ "$BOARD_ID" =~ ^b_[A-Za-z0-9]+$ ]] || return 2
    [[ "$LABEL_OUTBOX_ID" =~ ^[1-9][0-9]*$ ]] || return 2
    actual="$output_dir/$BOARD.delivery-proof.json"

    bounded_sqlite -readonly -json "$DB" "
      SELECT
        'task' AS canary_kind,e.id AS event_row_id,e.event_id,
        o.id AS outbox_id,d.id AS delivery_id,d.store_name,d.board_id,
        d.source_event_id,d.cursor,d.entity_uri,d.status,
        d.published_generation,s.active_generation,s.active_fence_epoch,
        s.fence_epoch AS store_fence_epoch,s.checkpoint_cursor
      FROM task_events e
      JOIN index_outbox o ON o.source_event_id=e.id
      JOIN projection_deliveries d
        ON d.outbox_id=o.id AND d.source_event_id=e.id
      JOIN projection_store_state s ON s.store_name=d.store_name
      WHERE e.task_id='$TASK_ID'
        AND e.board_id='$BOARD_ID'
        AND e.kind='task.created'
        AND e.actor='production-recovery'
        AND o.entity_uri='kb://task/$TASK_ID'
        AND d.store_name IN (
          'tantivy_tasks','oxigraph_relations','lancedb_chunks'
        )
      UNION ALL
      SELECT
        'label' AS canary_kind,NULL AS event_row_id,NULL AS event_id,
        o.id AS outbox_id,d.id AS delivery_id,d.store_name,d.board_id,
        d.source_event_id,d.cursor,d.entity_uri,d.status,
        d.published_generation,s.active_generation,s.active_fence_epoch,
        s.fence_epoch AS store_fence_epoch,s.checkpoint_cursor
      FROM index_outbox o
      JOIN projection_deliveries d ON d.outbox_id=o.id
      JOIN projection_store_state s ON s.store_name=d.store_name
      WHERE o.id=$LABEL_OUTBOX_ID
        AND o.source_event_id IS NULL
        AND o.projection_store='lancedb_label_atoms'
        AND o.entity_uri='kb://board/$BOARD_ID'
        AND d.store_name='lancedb_label_atoms'
        AND d.board_id='$BOARD_ID'
      ORDER BY canary_kind,store_name;
    " >"$actual" || return 2

    jq -e '
      type == "array" and length == 4 and
      ([.[] | select(.canary_kind == "task") | .store_name] | sort) ==
        ["lancedb_chunks","oxigraph_relations","tantivy_tasks"] and
      ([.[] | select(.canary_kind == "label") | .store_name]) ==
        ["lancedb_label_atoms"]
    ' "$actual" >/dev/null || return 2

    expected="$output_dir/$BOARD.delivery-identities.expected.json"
    jq -s '
      (.[0] | map({
        canary_kind:"task",event_row_id,event_id,outbox_id,delivery_id,
        store_name,board_id,source_event_id,cursor,entity_uri
      })) +
      (.[1] | map({
        canary_kind:"label",event_row_id:null,event_id:null,outbox_id,delivery_id,
        store_name,board_id,source_event_id,cursor,entity_uri
      })) | sort_by(.canary_kind,.store_name)
    ' "$EVIDENCE/canaries/$BOARD.task-delivery-ids.json" \
      "$EVIDENCE/canaries/$BOARD.label-delivery-id.json" >"$expected" \
      || return 2
    jq '
      map({
        canary_kind,event_row_id,event_id,outbox_id,delivery_id,
        store_name,board_id,source_event_id,cursor,entity_uri
      }) | sort_by(.canary_kind,.store_name)
    ' "$actual" >"$output_dir/$BOARD.delivery-identities.actual.json" \
      || return 2
    diff -u "$expected" \
      "$output_dir/$BOARD.delivery-identities.actual.json" \
      >"$output_dir/$BOARD.delivery-identities.diff" || return 3

    if jq -e '
      all(.[];
        .status == "done" and
        .published_generation != null and
        .published_generation == .active_generation and
        .cursor == .outbox_id and .cursor <= .checkpoint_cursor and
        .active_fence_epoch != null and
        .store_fence_epoch >= .active_fence_epoch
      )
    ' "$actual" >"$output_dir/$BOARD.delivery-ready.ok.txt"; then
      ready_boards=$((ready_boards + 1))
    fi
  done <"$EVIDENCE/canaries/boards.tsv"

  jq -s 'add' "$output_dir"/*.delivery-proof.json \
    >"$output_dir/all-deliveries.json" || return 2
  test "$(jq 'length' "$output_dir/all-deliveries.json")" -eq 36 || return 2
  test "$ready_boards" -eq 9
}
~~~

### 10.2 hard-stop 和 SLA poll

每个 poll 都从同一份 `maintenance status` JSON 检查精确四个 store，并重新执行全部 36
query 和 36 delivery rows。命令/JSON 错误、cross-board hit 或 ID drift 立即 stop owner、
采集 evidence 并以非零退出；120 秒未全部 PASS 同样 hard stop，不能只写一个失败文件后继续。
上一节在 owner 启动前安装的 trap 在这里继续生效，不得重置。

~~~bash
SLA_STARTED_EPOCH="$(date +%s)"
SLA_DEADLINE_EPOCH=$((SLA_STARTED_EPOCH + SLA_TIMEOUT_SEC))
printf 'started=%s deadline=%s poll_interval_sec=%s timeout_sec=%s\n' \
  "$SLA_STARTED_EPOCH" "$SLA_DEADLINE_EPOCH" \
  "$POLL_INTERVAL_SEC" "$SLA_TIMEOUT_SEC" \
  >"$EVIDENCE/sla/parameters.txt"

poll=0
while :; do
  poll=$((poll + 1))
  prefix="$EVIDENCE/sla/poll-$(printf '%03d' "$poll")"
  install -d -m 0700 "$prefix"
  bounded_kanban --db "$DB" --json maintenance status \
    >"$prefix/maintenance-status.json"
  bounded_kanban --db "$DB" --json outbox list --limit 1000 \
    >"$prefix/outbox.json"

  status_ready=0
  now_ms=$(( $(date +%s) * 1000 ))
  if jq -e --arg build_id \
    "$(jq -r '.build_id' "$COHORT/source-provenance.json")" \
    --argjson started "$OWNER_START_MS" --argjson now "$now_ms" '
    .data.maintenance_owner.active == true and
    .data.maintenance_owner.mode == "continuous" and
    .data.maintenance_owner.build_identity == $build_id and
    (.data.maintenance_owner.last_heartbeat_at | type) == "number" and
    .data.maintenance_owner.last_heartbeat_at >= $started and
    .data.maintenance_owner.last_heartbeat_at <= $now and
    .data.maintenance_owner.lease_expires_at > $now and
    (.data.maintenance_owner.capabilities | sort) ==
      ["lancedb_chunks","lancedb_label_atoms",
       "oxigraph_relations","tantivy_tasks"] and
    (.data.stores | length) == 4 and
    ([.data.stores[].store_name] | sort) ==
      ["lancedb_chunks","lancedb_label_atoms",
       "oxigraph_relations","tantivy_tasks"] and
    all(.data.stores[];
      .lifecycle_status == "ready" and
      .runtime_availability == "available" and
      .fallback_reason == null and .last_error == null and
      .pending == 0 and .running == 0 and .failed == 0 and .legacy_done == 0 and
      .active_generation != null and .building_generation == null
    )
  ' "$prefix/maintenance-status.json" >"$prefix/four-stores-ready.ok.txt"; then
    status_ready=1
  fi

  matrix_ready=0
  if run_query_matrix "$prefix/queries" "$prefix/matrix.assertions.tsv"; then
    matrix_ready=1
  else
    matrix_status=$?
    case "$matrix_status" in
      1) ;;
      2) hard_stop "query-command-or-contract-error poll=$poll" ;;
      3) hard_stop "query-cross-board-or-safety-error poll=$poll" ;;
      *) hard_stop "unexpected-query-status=$matrix_status poll=$poll" ;;
    esac
  fi

  deliveries_ready=0
  if run_delivery_proof "$prefix/deliveries"; then
    deliveries_ready=1
  else
    delivery_status=$?
    case "$delivery_status" in
      1) ;;
      2) hard_stop "delivery-query-or-contract-error poll=$poll" ;;
      3) hard_stop "delivery-identity-drift poll=$poll" ;;
      *) hard_stop "unexpected-delivery-status=$delivery_status poll=$poll" ;;
    esac
  fi

  now="$(date +%s)"
  printf 'poll=%03d epoch=%s elapsed_sec=%s status=%s matrix=%s deliveries=%s\n' \
    "$poll" "$now" "$((now - SLA_STARTED_EPOCH))" \
    "$status_ready" "$matrix_ready" "$deliveries_ready" \
    >>"$EVIDENCE/sla/polls.txt"
  if test "$status_ready" -eq 1 &&
    test "$matrix_ready" -eq 1 &&
    test "$deliveries_ready" -eq 1 &&
    test "$now" -le "$SLA_DEADLINE_EPOCH"; then
    cp --no-preserve=mode "$prefix/matrix.assertions.tsv" \
      "$EVIDENCE/queries/matrix.assertions.tsv"
    cp -R --no-preserve=mode "$prefix/queries/." "$EVIDENCE/queries/"
    printf 'met_sla=true poll=%03d elapsed_sec=%s\n' \
      "$poll" "$((now - SLA_STARTED_EPOCH))" \
      | tee "$EVIDENCE/sla/result.txt"
    break
  fi
  if test "$now" -ge "$SLA_DEADLINE_EPOCH"; then
    hard_stop "sla-timeout poll=$poll elapsed=$((now - SLA_STARTED_EPOCH))"
  fi
  next_poll_epoch=$((SLA_STARTED_EPOCH + poll * POLL_INTERVAL_SEC))
  sleep_for=$((next_poll_epoch - now))
  if test "$sleep_for" -gt 0; then
    sleep "$sleep_for"
  fi
done

test "$(tail -n +2 "$EVIDENCE/queries/matrix.assertions.tsv" | wc -l)" -eq 36
test "$(
  tail -n +2 "$EVIDENCE/queries/matrix.assertions.tsv" \
    | cut -f1,2 | sort -u | wc -l
)" -eq 36
awk -F '\t' 'NR > 1 && ($6 <= 0 || $8 != 0 || $9 != "PASS") { exit 1 }' \
  "$EVIDENCE/queries/matrix.assertions.tsv"
~~~

## 11. owner restart：新 PID/heartbeat/lease/fence 和 post-restart mutation

restart 前只保存基线；mutation 必须发生在新 PID、fresh heartbeat 和新 singleton lease 已
确认之后。post-restart update 使用每次唯一的新 description，响应必须证明值实际改变。
随后以 task event 的精确三路 delivery、三个 task store 的 fence 递增、Tantivy 唯一文本
命中和 exact task/ref/board/value 收敛作为 restart 验收。

~~~bash
bounded_kanban --db "$DB" --json maintenance status \
  >"$EVIDENCE/restart/status.before.json"
OLD_PID="$(
  bounded_sudo systemctl show "$MAINTENANCE_UNIT" \
    --property=MainPID --value
)"
[[ "$OLD_PID" =~ ^[1-9][0-9]*$ ]]
printf '%s\n' "$OLD_PID" >"$EVIDENCE/restart/main-pid.before.txt"
OLD_HEARTBEAT="$(
  jq -er '.data.maintenance_owner.last_heartbeat_at' \
    "$EVIDENCE/restart/status.before.json"
)"
[[ "$OLD_HEARTBEAT" =~ ^[0-9]+$ ]]

RESTART_STARTED_MS=$(( $(date +%s) * 1000 ))
bounded_sudo systemctl restart "$MAINTENANCE_UNIT"
RESTART_DEADLINE=$(( $(date +%s) + OWNER_START_TIMEOUT_SEC ))
while :; do
  NEW_PID="$(
    bounded_sudo systemctl show "$MAINTENANCE_UNIT" \
      --property=MainPID --value
  )"
  bounded_kanban --db "$DB" --json maintenance status \
    >"$EVIDENCE/restart/status.after.current.json"
  now_ms=$(( $(date +%s) * 1000 ))
  if [[ "$NEW_PID" =~ ^[1-9][0-9]*$ ]] &&
    test "$NEW_PID" != "$OLD_PID" &&
    jq -e --arg build_id \
      "$(jq -r '.build_id' "$COHORT/source-provenance.json")" \
      --argjson started "$RESTART_STARTED_MS" --argjson now "$now_ms" '
      .data.maintenance_owner.active == true and
      .data.maintenance_owner.mode == "continuous" and
      .data.maintenance_owner.build_identity == $build_id and
      (.data.maintenance_owner.last_heartbeat_at | type) == "number" and
      .data.maintenance_owner.last_heartbeat_at >= $started and
      .data.maintenance_owner.last_heartbeat_at <= $now and
      .data.maintenance_owner.lease_expires_at > $now and
      (.data.maintenance_owner.capabilities | sort) ==
        ["lancedb_chunks","lancedb_label_atoms",
         "oxigraph_relations","tantivy_tasks"]
    ' "$EVIDENCE/restart/status.after.current.json" >/dev/null; then
    break
  fi
  test "$(date +%s)" -lt "$RESTART_DEADLINE"
  sleep 1
done
cp --no-preserve=mode "$EVIDENCE/restart/status.after.current.json" \
  "$EVIDENCE/restart/status.after-owner-ready.json"
printf '%s\n' "$NEW_PID" >"$EVIDENCE/restart/main-pid.after.txt"
assert_non_owner_writers_stopped
assert_live_unit_binding "$NEW_PID" \
  "$EVIDENCE/restart/live-helper-environment.redacted.json"

read -r RESTART_BOARD RESTART_BOARD_ID \
  < <(head -n 1 "$EVIDENCE/canaries/boards.tsv")
RESTART_TASK_ID="$(
  jq -er '.data.id' "$EVIDENCE/canaries/$RESTART_BOARD.task-create.json"
)"
RESTART_TASK_REF="$(
  jq -er '.data.ref' "$EVIDENCE/canaries/$RESTART_BOARD.task-create.json"
)"
[[ "$RESTART_BOARD_ID" =~ ^b_[A-Za-z0-9]+$ ]]
[[ "$RESTART_TASK_ID" =~ ^t_[A-Za-z0-9]+$ ]]
bounded_sqlite -readonly -json "$DB" \
  'SELECT COALESCE(MAX(id),0) AS max_id FROM task_events;' \
  >"$EVIDENCE/restart/event-baseline.json"
RESTART_EVENT_BASELINE="$(
  jq -er '.[0].max_id' "$EVIDENCE/restart/event-baseline.json"
)"
[[ "$RESTART_EVENT_BASELINE" =~ ^[0-9]+$ ]]
RESTART_VALUE="ownerrestartcanary${RECOVERY_ID//[^A-Za-z0-9]/}$(date +%s%N)"

bounded_kanban --db "$DB" --board "$RESTART_BOARD" \
  --actor production-recovery --json \
  task update "$RESTART_TASK_REF" --description "$RESTART_VALUE" \
  >"$EVIDENCE/restart/task-update.json"
jq -e --arg id "$RESTART_TASK_ID" --arg ref "$RESTART_TASK_REF" \
  --arg board_id "$RESTART_BOARD_ID" --arg value "$RESTART_VALUE" '
  .data.id == $id and .data.ref == $ref and
  .data.board_id == $board_id and .data.description == $value
' "$EVIDENCE/restart/task-update.json" \
  >"$EVIDENCE/restart/task-update.changed.ok.txt"

bounded_sqlite -readonly -json "$DB" "
  SELECT
    e.id AS event_row_id,e.event_id,o.id AS outbox_id,
    d.id AS delivery_id,d.store_name,d.board_id,d.source_event_id,
    d.cursor,d.entity_uri
  FROM task_events e
  JOIN index_outbox o ON o.source_event_id=e.id
  JOIN projection_deliveries d
    ON d.outbox_id=o.id AND d.source_event_id=e.id
  WHERE e.id > $RESTART_EVENT_BASELINE
    AND e.task_id='$RESTART_TASK_ID'
    AND e.board_id='$RESTART_BOARD_ID'
    AND e.kind='task.updated'
    AND e.actor='production-recovery'
    AND o.entity_uri='kb://task/$RESTART_TASK_ID'
    AND d.store_name IN (
      'tantivy_tasks','oxigraph_relations','lancedb_chunks'
    )
  ORDER BY d.store_name;
" >"$EVIDENCE/restart/delivery-ids.json"
jq -e '
  length == 3 and
  ([.[].store_name] | sort) ==
    ["lancedb_chunks","oxigraph_relations","tantivy_tasks"] and
  ([.[].event_row_id] | unique | length) == 1 and
  ([.[].event_id] | unique | length) == 1 and
  ([.[].outbox_id] | unique | length) == 3 and
  ([.[].delivery_id] | unique | length) == 3
' "$EVIDENCE/restart/delivery-ids.json" \
  >"$EVIDENCE/restart/delivery-ids.ok.txt"
~~~

restart convergence 仍使用相同 5s/120s 界限；任何命令错误或 timeout 调用上一节
`hard_stop` 并非零退出。

~~~bash
RESTART_SLA_STARTED="$(date +%s)"
RESTART_SLA_DEADLINE=$((RESTART_SLA_STARTED + SLA_TIMEOUT_SEC))
restart_poll=0
while :; do
  restart_poll=$((restart_poll + 1))
  prefix="$EVIDENCE/restart/poll-$(printf '%03d' "$restart_poll")"
  install -d -m 0700 "$prefix"
  bounded_kanban --db "$DB" --json maintenance status \
    >"$prefix/maintenance-status.json"
  bounded_kanban --db "$DB" --board "$RESTART_BOARD" --json \
    search "$RESTART_VALUE" --limit 20 >"$prefix/task-search.json"

  RESTART_EVENT_ROW_ID="$(
    jq -er '.[0].event_row_id' "$EVIDENCE/restart/delivery-ids.json"
  )"
  [[ "$RESTART_EVENT_ROW_ID" =~ ^[1-9][0-9]*$ ]]
  bounded_sqlite -readonly -json "$DB" "
    SELECT
      e.id AS event_row_id,e.event_id,o.id AS outbox_id,
      d.id AS delivery_id,d.store_name,d.board_id,d.source_event_id,
      d.cursor,d.status,d.published_generation,
      s.active_generation,s.active_fence_epoch,
      s.fence_epoch AS store_fence_epoch,s.checkpoint_cursor
    FROM task_events e
    JOIN index_outbox o ON o.source_event_id=e.id
    JOIN projection_deliveries d
      ON d.outbox_id=o.id AND d.source_event_id=e.id
    JOIN projection_store_state s ON s.store_name=d.store_name
    WHERE e.id=$RESTART_EVENT_ROW_ID
      AND d.store_name IN (
        'tantivy_tasks','oxigraph_relations','lancedb_chunks'
      )
    ORDER BY d.store_name;
  " >"$prefix/deliveries.json"

  jq '
    map({
      event_row_id,event_id,outbox_id,delivery_id,store_name,board_id,
      source_event_id,cursor
    }) | sort_by(.store_name)
  ' "$prefix/deliveries.json" >"$prefix/delivery-ids.actual.json"
  jq '
    map({
      event_row_id,event_id,outbox_id,delivery_id,store_name,board_id,
      source_event_id,cursor
    }) | sort_by(.store_name)
  ' "$EVIDENCE/restart/delivery-ids.json" \
    >"$prefix/delivery-ids.expected.json"
  diff -u "$prefix/delivery-ids.expected.json" \
    "$prefix/delivery-ids.actual.json" >"$prefix/delivery-ids.diff"

  now_ms=$(( $(date +%s) * 1000 ))
  owner_ready=0
  if jq -e --arg build_id \
    "$(jq -r '.build_id' "$COHORT/source-provenance.json")" \
    --argjson started "$RESTART_STARTED_MS" --argjson now "$now_ms" '
    .data.maintenance_owner.active == true and
    .data.maintenance_owner.mode == "continuous" and
    .data.maintenance_owner.build_identity == $build_id and
    (.data.maintenance_owner.last_heartbeat_at | type) == "number" and
    .data.maintenance_owner.last_heartbeat_at >= $started and
    .data.maintenance_owner.last_heartbeat_at <= $now and
    .data.maintenance_owner.lease_expires_at > $now and
    (.data.maintenance_owner.capabilities | sort) ==
      ["lancedb_chunks","lancedb_label_atoms",
       "oxigraph_relations","tantivy_tasks"] and
    (.data.stores | length) == 4 and
    all(.data.stores[];
      .lifecycle_status == "ready" and
      .runtime_availability == "available" and
      .fallback_reason == null and
      .pending == 0 and .running == 0 and .failed == 0
    )
  ' "$prefix/maintenance-status.json" >/dev/null; then
    owner_ready=1
  fi

  delivery_ready=0
  if jq -e '
    length == 3 and
    ([.[].store_name] | sort) ==
      ["lancedb_chunks","oxigraph_relations","tantivy_tasks"] and
    all(.[];
      .status == "done" and
      .published_generation == .active_generation and
      .cursor == .outbox_id and .cursor <= .checkpoint_cursor and
      .active_fence_epoch != null and
      .store_fence_epoch >= .active_fence_epoch
    )
  ' "$prefix/deliveries.json" >/dev/null; then
    delivery_ready=1
  fi

  fence_ready=0
  if jq -e --slurpfile before "$EVIDENCE/restart/status.before.json" '
    ($before[0].data.stores |
      map({key:.store_name,value:.fence_epoch}) | from_entries) as $old |
    all(.data.stores[];
      if (.store_name == "tantivy_tasks" or
          .store_name == "oxigraph_relations" or
          .store_name == "lancedb_chunks")
      then .fence_epoch > $old[.store_name]
      else .fence_epoch >= $old[.store_name]
      end
    )
  ' "$prefix/maintenance-status.json" >/dev/null; then
    fence_ready=1
  fi

  search_ready=0
  if jq -e --arg id "$RESTART_TASK_ID" --arg ref "$RESTART_TASK_REF" \
    --arg board_id "$RESTART_BOARD_ID" --arg value "$RESTART_VALUE" '
    .meta.backend == "tantivy" and .meta.stale == false and
    .meta.resolved_board_id == $board_id and
    ([.data.hits[] |
      select(.task.id == $id and .task.ref == $ref and
             .task.board_id == $board_id and
             .task.description == $value)] | length) == 1 and
    ([.data.hits[] | select(.task.board_id != $board_id)] | length) == 0
  ' "$prefix/task-search.json" >/dev/null; then
    search_ready=1
  fi

  current_pid="$(
    bounded_sudo systemctl show "$MAINTENANCE_UNIT" \
      --property=MainPID --value
  )"
  test "$current_pid" = "$NEW_PID"
  now="$(date +%s)"
  printf 'poll=%03d epoch=%s owner=%s delivery=%s fence=%s search=%s\n' \
    "$restart_poll" "$now" "$owner_ready" "$delivery_ready" \
    "$fence_ready" "$search_ready" >>"$EVIDENCE/restart/polls.txt"
  if test "$owner_ready" -eq 1 &&
    test "$delivery_ready" -eq 1 &&
    test "$fence_ready" -eq 1 &&
    test "$search_ready" -eq 1 &&
    test "$now" -le "$RESTART_SLA_DEADLINE"; then
    printf 'met_sla=true poll=%03d elapsed_sec=%s\n' \
      "$restart_poll" "$((now - RESTART_SLA_STARTED))" \
      | tee "$EVIDENCE/restart/result.txt"
    break
  fi
  if test "$now" -ge "$RESTART_SLA_DEADLINE"; then
    hard_stop \
      "restart-sla-timeout poll=$restart_poll elapsed=$((now - RESTART_SLA_STARTED))"
  fi
  next_poll_epoch=$((RESTART_SLA_STARTED + restart_poll * POLL_INTERVAL_SEC))
  sleep_for=$((next_poll_epoch - now))
  if test "$sleep_for" -gt 0; then
    sleep "$sleep_for"
  fi
done
~~~

## 12. cleanup 前 doctor、binding、matrix 和 delivery gate

cleanup 准入 gate 不接受“命令退出 0”这一条证据。`doctor.data.ok`、migration/integrity、
consistency/ontology、全部 canonical hard-error counters、derived counters、owner binding、
四-store final expected diff、36-query matrix 和 36-row delivery proof 必须同时通过。
同一个函数必须在 cleanup 完成并重启 owner 后原样再跑一次；第二次才是最终验收。

~~~bash
run_final_owner_binding_health_gate() {
  local phase="$1" output_dir="$2" owner_pid
  install -d -m 0700 "$output_dir"

  bounded_sudo systemctl is-active --quiet "$MAINTENANCE_UNIT"
  owner_pid="$(
    bounded_sudo systemctl show "$MAINTENANCE_UNIT" \
      --property=MainPID --value
  )"
  [[ "$owner_pid" =~ ^[1-9][0-9]*$ ]]
  assert_non_owner_writers_stopped
  assert_live_unit_binding "$owner_pid" \
    "$output_dir/live-helper-environment.redacted.json"

  bounded_kanban --db "$DB" --json doctor --strict-derived \
    | tee "$output_dir/doctor.strict-derived.json"
  bounded_kanban --db "$DB" --json maintenance status \
    | tee "$output_dir/maintenance-status.json"
  bounded_kanban --db "$DB" --json outbox list --limit 1000 \
    | tee "$output_dir/outbox.json"

  jq -e '
    .data.ok == true and .data.integrity_check == "ok" and
    .data.migration_version == 30 and .data.user_version == 30 and
    .data.expired_running_tasks == 0 and
    .data.running_tasks_without_active_run == 0 and
    .data.orphan_running_runs == 0 and
    .data.dependency_cycles == 0 and
    .data.archived_dependency_edges == 0 and
    .data.missing_run_logs == 0 and
    .data.suspicious_run_log_paths == 0 and
    .data.executable_dependency_violations == 0 and
    .data.executable_spec_violations == 0 and
    .data.executable_schedule_violations == 0 and
    .data.outbox_pending == 0 and
    .data.outbox_running == 0 and
    .data.outbox_failed == 0 and
    .data.derived_dirty_stores == 0 and
    .data.derived_error_stores == 0 and
    (.data.derived_stores | length) == 4 and
    all(.data.derived_stores[];
      .dirty == false and .last_error == null and
      .pending_outbox == 0 and .running_outbox == 0 and .failed_outbox == 0
    ) and
    .data.consistency_errors == 0 and
    ([.data.consistency_issues[] | select(.severity == "error")] | length) == 0 and
    .data.ontology_ledger_errors == 0 and
    ([.data.ontology_ledger_issues[] | select(.severity == "error")] | length) == 0
  ' "$output_dir/doctor.strict-derived.json" \
    >"$output_dir/doctor.exact.ok.txt"

  jq -e --arg build_id \
    "$(jq -r '.build_id' "$COHORT/source-provenance.json")" \
    --argjson now "$(( $(date +%s) * 1000 ))" '
    .data.maintenance_owner.active == true and
    .data.maintenance_owner.mode == "continuous" and
    .data.maintenance_owner.build_identity == $build_id and
    (.data.maintenance_owner.last_heartbeat_at | type) == "number" and
    .data.maintenance_owner.last_heartbeat_at <= $now and
    .data.maintenance_owner.lease_expires_at > $now and
    (.data.maintenance_owner.capabilities | sort) ==
      ["lancedb_chunks","lancedb_label_atoms",
       "oxigraph_relations","tantivy_tasks"]
  ' "$output_dir/maintenance-status.json" \
    >"$output_dir/owner-binding.ok.txt"

  assert_final_store_state "$phase" "$output_dir/maintenance-status.json"
  run_query_matrix "$output_dir/queries" "$output_dir/matrix.assertions.tsv"
  run_delivery_proof "$output_dir/deliveries"
  assert_release_binding "$phase"
  test "$(
    bounded_sudo systemctl show "$MAINTENANCE_UNIT" \
      --property=MainPID --value
  )" = "$owner_pid"
  bounded_sudo systemctl is-active --quiet "$MAINTENANCE_UNIT"
  assert_non_owner_writers_stopped
  assert_live_unit_binding "$owner_pid" \
    "$output_dir/live-helper-environment.after-gate.redacted.json"
}

run_final_owner_binding_health_gate \
  pre-cleanup "$EVIDENCE/final/pre-cleanup"
~~~

## 13. Legacy cleanup 和 rollback 边界

只有第 9—12 节全部通过，才允许使用最初 inventory digest 管理固定 legacy allowlist。
continuous owner 持有 singleton 时绝不能直接调用 cleanup。必须先保存其 active/continuous
身份，停止 owner，等待并精确断言 singleton session 与四个 store lease 全部释放，同时
要求 inventory 中所有 writer 为 `inactive`/absent 且数据库没有 holder。`apply` 与
`verify` 命令各自使用 CLI 内建的 `MaintenanceMode::Once` session；每条命令返回后都必须
再次证明 session/lease 已回到 exact idle，才能继续。

~~~bash
CLEANUP_DIR="$EVIDENCE/final/cleanup"
install -d -m 0700 "$CLEANUP_DIR"
bounded_sudo systemctl is-active --quiet "$MAINTENANCE_UNIT"
CLEANUP_OLD_PID="$(
  bounded_sudo systemctl show "$MAINTENANCE_UNIT" \
    --property=MainPID --value
)"
[[ "$CLEANUP_OLD_PID" =~ ^[1-9][0-9]*$ ]]
bounded_kanban --db "$DB" --json maintenance status \
  >"$CLEANUP_DIR/continuous-owner.before-stop.json"
jq -e --arg build_id \
  "$(jq -r '.build_id' "$COHORT/source-provenance.json")" \
  --argjson now "$(( $(date +%s) * 1000 ))" '
  .data.maintenance_owner.active == true and
  .data.maintenance_owner.mode == "continuous" and
  .data.maintenance_owner.build_identity == $build_id and
  .data.maintenance_owner.lease_expires_at > $now and
  (.data.maintenance_owner.capabilities | sort) ==
    ["lancedb_chunks","lancedb_label_atoms",
     "oxigraph_relations","tantivy_tasks"]
' "$CLEANUP_DIR/continuous-owner.before-stop.json" \
  >"$CLEANUP_DIR/continuous-owner.before-stop.ok.txt"
assert_live_unit_binding "$CLEANUP_OLD_PID" \
  "$CLEANUP_DIR/continuous-owner.live-binding.before-stop.json"

bounded_sudo systemctl stop "$MAINTENANCE_UNIT"
bounded_sudo systemctl reset-failed "$MAINTENANCE_UNIT"
assert_writers_stopped "$CLEANUP_DIR/writers.after-owner-stop.txt"
wait_for_exact_maintenance_release "$CLEANUP_DIR" after-owner-stop
assert_writers_stopped "$CLEANUP_DIR/writers.after-owner-release.txt"
assert_no_database_holders "$CLEANUP_DIR/database-holders.after-owner-stop.txt"

bounded_kanban --db "$DB" --actor production-recovery --json \
  maintenance cleanup-legacy apply \
  --backup-dir "$EVIDENCE/backup/legacy-projections" \
  --expected-inventory-digest "$LEGACY_DIGEST" \
  | tee "$CLEANUP_DIR/legacy-cleanup.apply.json"
jq -e --arg db "$DB_INSTANCE_ID" --arg path "$DB" \
  --arg backup "$EVIDENCE/backup/legacy-projections" \
  --arg digest "$LEGACY_DIGEST" '
  .data.action == "apply" and .data.dry_run == false and
  .data.resumed == false and .data.format_version == 1 and
  .data.database_instance_id == $db and .data.database_path == $path and
  .data.backup_dir == $backup and .data.inventory_digest == $digest and
  (.data.roots | length) == 5 and
  ([.data.roots[].relative_path] | sort) ==
    ["index/v1/graph","index/v1/tasks","index/v1/vectors",
     "index/v2/oxigraph_relations","index/v2/tantivy_tasks"]
' "$CLEANUP_DIR/legacy-cleanup.apply.json" \
  >"$CLEANUP_DIR/legacy-cleanup.apply.ok.txt"
wait_for_exact_maintenance_release "$CLEANUP_DIR" after-apply-once
assert_writers_stopped "$CLEANUP_DIR/writers.after-apply-once.txt"
assert_no_database_holders "$CLEANUP_DIR/database-holders.after-apply-once.txt"

bounded_kanban --db "$DB" --actor production-recovery --json \
  maintenance cleanup-legacy verify \
  --backup-dir "$EVIDENCE/backup/legacy-projections" \
  | tee "$CLEANUP_DIR/legacy-cleanup.verify.json"
jq -e --arg db "$DB_INSTANCE_ID" --arg path "$DB" \
  --arg backup "$EVIDENCE/backup/legacy-projections" \
  --arg digest "$LEGACY_DIGEST" '
  .data.action == "verify" and .data.dry_run == false and
  .data.resumed == false and .data.format_version == 1 and
  .data.database_instance_id == $db and .data.database_path == $path and
  .data.backup_dir == $backup and .data.inventory_digest == $digest and
  (.data.roots | length) == 5 and
  ([.data.roots[].relative_path] | sort) ==
    ["index/v1/graph","index/v1/tasks","index/v1/vectors",
     "index/v2/oxigraph_relations","index/v2/tantivy_tasks"]
' "$CLEANUP_DIR/legacy-cleanup.verify.json" \
  >"$CLEANUP_DIR/legacy-cleanup.verify.ok.txt"
jq -S '.data | del(.action)' "$CLEANUP_DIR/legacy-cleanup.apply.json" \
  >"$CLEANUP_DIR/legacy-cleanup.apply.comparable.json"
jq -S '.data | del(.action)' "$CLEANUP_DIR/legacy-cleanup.verify.json" \
  >"$CLEANUP_DIR/legacy-cleanup.verify.comparable.json"
diff -u "$CLEANUP_DIR/legacy-cleanup.apply.comparable.json" \
  "$CLEANUP_DIR/legacy-cleanup.verify.comparable.json" \
  >"$CLEANUP_DIR/legacy-cleanup.apply-verify.diff"
wait_for_exact_maintenance_release "$CLEANUP_DIR" after-verify-once
assert_writers_stopped "$CLEANUP_DIR/writers.after-verify-once.txt"
assert_no_database_holders "$CLEANUP_DIR/database-holders.after-verify-once.txt"
~~~

cleanup apply 只移动固定 allowlist；`index/v2/databases/<database_instance_id>/...` 永远不在
allowlist。journal 中断时先验证 DB identity、backup binding 和 digest，再按命令契约显式
使用 resume；不要删除 journal、backup 或 previous generation。

cleanup 完成后必须显式重新启动 continuous owner。新 MainPID 必须不同于 cleanup 前 PID；
readiness 必须证明启动后的 fresh heartbeat/lease、同 cohort build identity、四项 capability
和四个 store 的 runtime availability。随后原样重跑第 12 节的完整
owner/binding/health gate；这里不得只检查 `systemctl is-active`。

~~~bash
assert_writers_stopped "$CLEANUP_DIR/writers.before-owner-restart.txt"
CLEANUP_RESTART_STARTED_MS=$(( $(date +%s) * 1000 ))
bounded_sudo systemctl start "$MAINTENANCE_UNIT"
CLEANUP_RESTART_DEADLINE=$(( $(date +%s) + OWNER_START_TIMEOUT_SEC ))
while :; do
  CLEANUP_RESTART_PID="$(
    bounded_sudo systemctl show "$MAINTENANCE_UNIT" \
      --property=MainPID --value
  )"
  bounded_kanban --db "$DB" --json maintenance status \
    >"$CLEANUP_DIR/owner-restart.status.current.json"
  now_ms=$(( $(date +%s) * 1000 ))
  if [[ "$CLEANUP_RESTART_PID" =~ ^[1-9][0-9]*$ ]] &&
    test "$CLEANUP_RESTART_PID" != "$CLEANUP_OLD_PID" &&
    jq -e --arg build_id \
      "$(jq -r '.build_id' "$COHORT/source-provenance.json")" \
      --argjson started "$CLEANUP_RESTART_STARTED_MS" --argjson now "$now_ms" '
      .data.maintenance_owner.active == true and
      .data.maintenance_owner.mode == "continuous" and
      .data.maintenance_owner.build_identity == $build_id and
      .data.maintenance_owner.last_heartbeat_at >= $started and
      .data.maintenance_owner.lease_expires_at > $now and
      (.data.maintenance_owner.capabilities | sort) ==
        ["lancedb_chunks","lancedb_label_atoms",
         "oxigraph_relations","tantivy_tasks"] and
      (.data.stores | length) == 4 and
      ([.data.stores[].store_name] | sort) ==
        ["lancedb_chunks","lancedb_label_atoms",
         "oxigraph_relations","tantivy_tasks"] and
      all(.data.stores[];
        .runtime_availability == "available" and
        .lifecycle_status == "ready" and
        .fallback_reason == null and .last_error == null
      )
    ' "$CLEANUP_DIR/owner-restart.status.current.json" >/dev/null; then
    break
  fi
  test "$(date +%s)" -lt "$CLEANUP_RESTART_DEADLINE"
  sleep 1
done
cp --no-preserve=mode "$CLEANUP_DIR/owner-restart.status.current.json" \
  "$CLEANUP_DIR/owner-restart.status.ready.json"
printf '%s\n' "$CLEANUP_RESTART_PID" \
  >"$CLEANUP_DIR/owner-restart.main-pid.txt"
bounded_sudo systemctl is-active --quiet "$MAINTENANCE_UNIT"
assert_non_owner_writers_stopped
assert_live_unit_binding "$CLEANUP_RESTART_PID" \
  "$CLEANUP_DIR/owner-restart.live-binding.json"

run_final_owner_binding_health_gate \
  post-cleanup-final "$EVIDENCE/final/post-cleanup-owner-restart"
~~~

Projection v2 没有 previous-generation republish API：

- 失败 generation 先保留，owner 先停止；
- 有 `building_generation` 时只能对同一 store 使用 dry-run 后的 `--resume`；
- 没有 building generation 时只能启动新的普通 rebuild，或保持停止并 read-only escalate；
- cleanup restore 只恢复 legacy allowlist，不恢复 v2 active/previous generation；
- canonical 灾难恢复是独立、高风险 database replacement，必须使用已校验 backup 和
  exclusive lifecycle authority，不能由 derived cleanup 或 generation rollback 代替。

## 14. 权限收口和 hash closure

证据根从创建起就是 `0700`；封存前再次把目录收紧为 `0700`、文件收紧为 `0600`。这不会
修改 cohort 或生产数据。canonical backup 和 hash 必须继续是 `0600`。manifest 排除自身与
随后生成的自校验文件，避免自引用。

~~~bash
(
  cd "$EVIDENCE/backup"
  sha256sum --check canonical.sqlite.sha256
) | tee "$EVIDENCE/backup/canonical.sqlite.sha256.check.final.txt"

find "$EVIDENCE" -type d -exec chmod 0700 {} +
find "$EVIDENCE" -type f -exec chmod 0600 {} +
test "$(stat -c '%a' "$EVIDENCE")" = 700
test "$(stat -c '%a' "$EVIDENCE/backup/canonical.sqlite")" = 600
test "$(stat -c '%a' "$EVIDENCE/backup/canonical.sqlite.sha256")" = 600
find "$EVIDENCE" -type d ! -perm 0700 -print \
  >"$EVIDENCE/final/non-0700-directories.txt"
find "$EVIDENCE" -type f ! -perm 0600 -print \
  >"$EVIDENCE/final/non-0600-files.txt"
test ! -s "$EVIDENCE/final/non-0700-directories.txt"
test ! -s "$EVIDENCE/final/non-0600-files.txt"

(
  cd "$EVIDENCE"
  find . -type f \
    ! -name evidence.sha256 \
    ! -name evidence.sha256.check.txt \
    -print0 \
    | sort -z \
    | while IFS= read -r -d '' file; do sha256sum "$file"; done
) >"$EVIDENCE/evidence.sha256"
chmod 0600 "$EVIDENCE/evidence.sha256"
(
  cd "$EVIDENCE"
  sha256sum --check evidence.sha256
) | tee "$EVIDENCE/evidence.sha256.check.txt"
chmod 0600 "$EVIDENCE/evidence.sha256.check.txt"
~~~

此后才可以把 recovery record 标为 complete。若任一 machine assertion、exact diff、
36-row matrix、delivery/fence proof、doctor 字段、权限或 hash 漂移，标为 blocked，
调用 `hard_stop` 并保持生产只读。
