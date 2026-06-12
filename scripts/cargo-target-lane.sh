#!/usr/bin/env bash
set -euo pipefail

TARGET_ROOT="${KANBAN_CARGO_TARGET_ROOT:-$HOME/.cache/kanban-tool/cargo-target}"
ALLOWED_LANES=(main cli server sqlite desktop)

usage() {
  cat <<'USAGE'
Usage:
  scripts/cargo-target-lane.sh <lane>
  scripts/cargo-target-lane.sh check <path>
  scripts/cargo-target-lane.sh list

Allowed lanes:
  main cli server sqlite desktop

Environment:
  KANBAN_CARGO_TARGET_ROOT  Override target root for local tests.
                            Default: $HOME/.cache/kanban-tool/cargo-target

Examples:
  scripts/cargo-target-lane.sh cli
  scripts/cargo-target-lane.sh check $HOME/.cache/kanban-tool/cargo-target/cli
USAGE
}

normalize_path() {
  local path="$1"
  while [[ "$path" != "/" && "$path" == */ ]]; do
    path="${path%/}"
  done
  printf '%s\n' "$path"
}

target_root() {
  normalize_path "$TARGET_ROOT"
}

print_allowed_lanes() {
  printf '%s\n' "${ALLOWED_LANES[@]}"
}

is_allowed_lane() {
  local lane="$1"
  local allowed

  for allowed in "${ALLOWED_LANES[@]}"; do
    if [[ "$lane" == "$allowed" ]]; then
      return 0
    fi
  done

  return 1
}

lane_path() {
  local lane="$1"
  printf '%s/%s\n' "$(target_root)" "$lane"
}

check_path() {
  local path
  local lane
  local candidate

  path="$(normalize_path "$1")"

  for lane in "${ALLOWED_LANES[@]}"; do
    candidate="$(lane_path "$lane")"
    if [[ "$path" == "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  echo "error: CARGO_TARGET_DIR is not an allowed kanban-tool lane: $1" >&2
  echo "error: allowed lanes are under $(target_root): ${ALLOWED_LANES[*]}" >&2
  return 2
}

main() {
  if [[ $# -lt 1 ]]; then
    usage >&2
    exit 2
  fi

  case "$1" in
    -h|--help)
      usage
      ;;
    list)
      print_allowed_lanes
      ;;
    check)
      if [[ $# -ne 2 ]]; then
        echo "error: check requires exactly one path" >&2
        usage >&2
        exit 2
      fi
      check_path "$2"
      ;;
    *)
      if [[ $# -ne 1 ]]; then
        echo "error: lane lookup accepts exactly one lane" >&2
        usage >&2
        exit 2
      fi
      if ! is_allowed_lane "$1"; then
        echo "error: unknown kanban-tool Cargo target lane: $1" >&2
        echo "error: allowed lanes: ${ALLOWED_LANES[*]}" >&2
        exit 2
      fi
      lane_path "$1"
      ;;
  esac
}

main "$@"
