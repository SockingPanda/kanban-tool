#!/usr/bin/env bash
set -euo pipefail
umask 077

ROOT="$(cd "$(dirname "$0")/.." && pwd -P)"
cd "$ROOT"
EVIDENCE="$ROOT/output/i18n"
PORT="${KANBAN_I18N_PORT:-18722}"
BASE_URL="http://127.0.0.1:$PORT"
[[ -f apps/web/dist/manifest.json ]] || { echo '请先执行 just web-build。' >&2; exit 1; }
mkdir -p "$EVIDENCE"
TEMP_ROOT="$(mktemp -d /tmp/kanban-i18n-proof.XXXXXX)"
DB_PATH="$TEMP_ROOT/canonical.db"
HOST_PID=""
HOST_START=""

process_start() { awk '{print $22}' "/proc/$HOST_PID/stat" 2>/dev/null || true; }
process_state() { awk '{print $3}' "/proc/$HOST_PID/stat" 2>/dev/null || true; }
host_matches() {
  [[ -n "$HOST_PID" && -n "$HOST_START" ]] && [[ "$(process_start)" == "$HOST_START" ]]
}
cleanup() {
  local status=$?
  trap - EXIT INT TERM
  if host_matches; then
    kill -INT "$HOST_PID" 2>/dev/null || true
    for _ in {1..100}; do
      host_matches || break
      [[ "$(process_state)" == Z ]] && break
      sleep 0.1
    done
    if host_matches && [[ "$(process_state)" != Z ]]; then
      kill -KILL "$HOST_PID" 2>/dev/null || true
    fi
    wait "$HOST_PID" 2>/dev/null || true
  fi
  [[ "$TEMP_ROOT" == /tmp/kanban-i18n-proof.* && -d "$TEMP_ROOT" && ! -L "$TEMP_ROOT" ]] && rm -rf -- "$TEMP_ROOT"
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# 在同一把锁内构建并复制可执行文件，避免其他 worktree 随后重建共享 binary。
scripts/cargo-build-lock.sh -- bash -c 'cargo build --locked -p kanban-cli && cp -- "$CARGO_TARGET_DIR/debug/kanban" "$1"' _ "$TEMP_ROOT/kanban"
KANBAN="$TEMP_ROOT/kanban"
KANBAN_SERVER_URL="$BASE_URL" KANBAN_ACTOR=i18n-proof "$KANBAN" --db "$DB_PATH" serve --host 127.0.0.1 --port "$PORT" --web-dir "$ROOT/apps/web/dist" > "$EVIDENCE/host.log" 2>&1 &
HOST_PID=$!
HOST_START="$(process_start)"

READY=false
for _ in {1..100}; do
  host_matches || break
  if curl --fail --silent --max-time 1 "$BASE_URL/health" > "$EVIDENCE/health.json" && jq -e --arg db "$DB_PATH" '.data.db_path == $db and .data.ok == true' "$EVIDENCE/health.json" >/dev/null; then
    READY=true
    break
  fi
  sleep 0.1
done
[[ "$READY" == true ]] || { echo "测试 Host 未就绪，参见 $EVIDENCE/host.log" >&2; exit 1; }

# CLI 仅经测试 Host 的 typed service path 创建夹具，不打开或写入数据库。
BOARDS="$(KANBAN_SERVER_URL="$BASE_URL" "$KANBAN" --json --board default board list)"
if ! jq -e '.data | any(.slug == "default")' <<< "$BOARDS" >/dev/null; then
  KANBAN_SERVER_URL="$BASE_URL" "$KANBAN" --json --board default board create default --name 'i18n 主项目' > "$EVIDENCE/primary-project.json"
fi
KANBAN_SERVER_URL="$BASE_URL" "$KANBAN" --json --board default board create i18n-secondary --name 'i18n 项目 <literal> {{name}}' > "$EVIDENCE/secondary-project.json"
for board in default i18n-secondary; do
  KANBAN_SERVER_URL="$BASE_URL" "$KANBAN" --json --board "$board" task create 'i18n 原文 <tag> $t(common:save)' --status todo > "$EVIDENCE/seed-$board.json"
done
curl --fail --silent "$BASE_URL/app/runtime.json" > "$EVIDENCE/runtime.json"
curl --fail --silent "$BASE_URL/app/manifest.json" > "$EVIDENCE/manifest.json"
EXPECTED_BUILD_ID="$(jq -er .buildId apps/web/dist/manifest.json)"
jq -e --arg expected "$EXPECTED_BUILD_ID" '.webBuildId == $expected' "$EVIDENCE/runtime.json" >/dev/null
jq -n --arg base "$(git rev-parse HEAD)" --arg tree "$(git rev-parse HEAD^{tree})" \
  --arg status "$(git status --porcelain --untracked-files=all)" \
  --arg diff "$(git diff HEAD --binary | sha256sum | cut -d ' ' -f 1)" \
  --arg build "$EXPECTED_BUILD_ID" --arg binary "$(sha256sum "$KANBAN" | cut -d ' ' -f 1)" \
  --argjson pid "$HOST_PID" --arg start "$HOST_START" --arg database "$DB_PATH" --arg url "$BASE_URL/app/" \
  '{baseCommit:$base,sourceTree:$tree,worktreeClean:($status==""),trackedDiffSha256:$diff,webBuildId:$build,hostBinarySha256:$binary,hostPid:$pid,hostStartTime:$start,testDatabase:$database,baseURL:$url}' > "$EVIDENCE/host-evidence.json"
KANBAN_I18N_EXPECTED_BUILD_ID="$EXPECTED_BUILD_ID" just web-i18n-e2e "$BASE_URL/app/"
# 同一候选 Host 上继续验证共享查询重连、原请求重试和任务附件的正式协议。
KANBAN_RELEASE_BASE_URL="$BASE_URL" KANBAN_RELEASE_PROOF=1 KANBAN_RELEASE_EXTENDED=1 \
  pnpm --filter @kanban-tool/web exec playwright test tests/release-grpc-migration.spec.ts \
  --project=chromium --output=../../output/playwright/grpc-integrated
