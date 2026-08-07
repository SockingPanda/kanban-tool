#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
JUST_VERSION="1.57.0"

log() {
  printf '==> %s\n' "$*"
}

expand_home_path() {
  local path="$1"

  case "$path" in
    '$HOME')
      printf '%s\n' "$HOME"
      ;;
    '$HOME/'*)
      printf '%s/%s\n' "$HOME" "${path#\$HOME/}"
      ;;
    '${HOME}')
      printf '%s\n' "$HOME"
      ;;
    '${HOME}/'*)
      printf '%s/%s\n' "$HOME" "${path#\$\{HOME\}/}"
      ;;
    '~')
      printf '%s\n' "$HOME"
      ;;
    '~/'*)
      printf '%s/%s\n' "$HOME" "${path#\~/}"
      ;;
    *)
      printf '%s\n' "$path"
      ;;
  esac
}

configure_paths() {
  CACHE_ROOT="$(expand_home_path "${KANBAN_CLOUD_CACHE_ROOT:-$HOME/.cache/kanban-tool}")"
  export KANBAN_CARGO_TARGET_ROOT
  KANBAN_CARGO_TARGET_ROOT="$(expand_home_path "${KANBAN_CARGO_TARGET_ROOT:-$CACHE_ROOT/cargo-target}")"
  export CARGO_TARGET_DIR="$KANBAN_CARGO_TARGET_ROOT"
  export KANBAN_CARGO_BUILD_JOBS="${KANBAN_CARGO_BUILD_JOBS:-auto}"
  export KANBAN_TEST_THREADS="${KANBAN_TEST_THREADS:-auto}"
  export PNPM_HOME
  PNPM_HOME="$(expand_home_path "${PNPM_HOME:-$HOME/.local/share/pnpm}")"
  export PATH="$KANBAN_CARGO_TARGET_ROOT/release:$KANBAN_CARGO_TARGET_ROOT/debug:$HOME/.cargo/bin:$PNPM_HOME:$PATH"
}

apt_get() {
  if [[ "${CODEX_CLOUD_SKIP_APT:-0}" == "1" ]]; then
    log "Skipping apt step because CODEX_CLOUD_SKIP_APT=1"
    return 0
  fi
  if ! command -v apt-get >/dev/null 2>&1; then
    log "apt-get not found; skipping Debian/Ubuntu system packages"
    return 0
  fi

  if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
    env DEBIAN_FRONTEND=noninteractive apt-get "$@"
  else
    sudo env DEBIAN_FRONTEND=noninteractive apt-get "$@"
  fi
}

install_protobuf_packages() {
  if command -v protoc >/dev/null 2>&1 && [[ -f /usr/include/google/protobuf/empty.proto ]]; then
    log "protobuf compiler and well-known types already installed"
    return 0
  fi
  if ! command -v apt-get >/dev/null 2>&1; then
    log "No apt-get available; protobuf package refresh skipped"
    return 0
  fi

  log "Installing protobuf compiler and well-known type definitions"
  apt_get update
  apt_get install -y --no-install-recommends protobuf-compiler libprotobuf-dev
}

main() {
  configure_paths
  mkdir -p "$CACHE_ROOT" "$KANBAN_CARGO_TARGET_ROOT" "$PNPM_HOME"
  install_protobuf_packages

  log "Refreshing Rust components"
  rustup component add rustfmt clippy

  if ! command -v just >/dev/null 2>&1 || [[ "$(just --version)" != "just $JUST_VERSION" ]]; then
    log "Installing just $JUST_VERSION"
    "$ROOT/scripts/cargo-build-lock.sh" -- cargo install just --version "$JUST_VERSION" --locked --force
  fi

  if [[ "${CODEX_CLOUD_INSTALL_NEXTEST:-1}" == "1" ]] && ! command -v cargo-nextest >/dev/null 2>&1; then
    log "Installing cargo-nextest"
    "$ROOT/scripts/cargo-build-lock.sh" -- cargo install cargo-nextest --locked
  fi

  if command -v corepack >/dev/null 2>&1; then
    log "Re-activating pnpm through corepack"
    corepack enable
    corepack prepare pnpm@10 --activate
  fi

  log "Refreshing desktop frontend dependencies"
  pnpm --dir "$ROOT/apps/desktop" install --frozen-lockfile

  log "Refreshing Rust dependency cache"
  (
    cd "$ROOT"
    "$ROOT/scripts/cargo-build-lock.sh" -- cargo fetch --locked
  )

  log "Codex Cloud maintenance complete"
}

main "$@"
