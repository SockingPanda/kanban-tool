#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CACHE_ROOT="${KANBAN_CLOUD_CACHE_ROOT:-$HOME/.cache/kanban-tool}"
export KANBAN_CARGO_TARGET_ROOT="${KANBAN_CARGO_TARGET_ROOT:-$CACHE_ROOT/cargo-target}"
export KANBAN_CARGO_BUILD_JOBS="${KANBAN_CARGO_BUILD_JOBS:-2}"
export KANBAN_TEST_THREADS="${KANBAN_TEST_THREADS:-2}"
export PNPM_HOME="${PNPM_HOME:-$HOME/.local/share/pnpm}"
export PATH="$HOME/.cargo/bin:$PNPM_HOME:$PATH"

log() {
  printf '==> %s\n' "$*"
}

main() {
  mkdir -p "$CACHE_ROOT" "$KANBAN_CARGO_TARGET_ROOT" "$PNPM_HOME"

  log "Refreshing Rust components"
  rustup component add rustfmt clippy

  if ! command -v just >/dev/null 2>&1; then
    log "Installing just"
    cargo install just --locked
  fi

  if [[ "${CODEX_CLOUD_INSTALL_NEXTEST:-1}" == "1" ]] && ! command -v cargo-nextest >/dev/null 2>&1; then
    log "Installing cargo-nextest"
    cargo install cargo-nextest --locked
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
    cargo fetch --locked
  )

  log "Codex Cloud maintenance complete"
}

main "$@"
