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

persist_shell_defaults() {
  local marker="# kanban-tool Codex Cloud defaults"
  local bashrc="$HOME/.bashrc"

  if [[ -f "$bashrc" ]] && grep -Fq "$marker" "$bashrc"; then
    return 0
  fi

  cat >>"$bashrc" <<'EOF'

# kanban-tool Codex Cloud defaults
export KANBAN_CARGO_TARGET_ROOT="${KANBAN_CARGO_TARGET_ROOT:-$HOME/.cache/kanban-tool/cargo-target}"
export KANBAN_CARGO_BUILD_JOBS="${KANBAN_CARGO_BUILD_JOBS:-2}"
export KANBAN_TEST_THREADS="${KANBAN_TEST_THREADS:-2}"
export PNPM_HOME="${PNPM_HOME:-$HOME/.local/share/pnpm}"
case ":$PATH:" in
  *":$HOME/.cargo/bin:"*) ;;
  *) export PATH="$HOME/.cargo/bin:$PATH" ;;
esac
case ":$PATH:" in
  *":$PNPM_HOME:"*) ;;
  *) export PATH="$PNPM_HOME:$PATH" ;;
esac
EOF
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

install_system_packages() {
  if [[ "${CODEX_CLOUD_INSTALL_TAURI_DEPS:-1}" != "1" ]]; then
    log "Skipping Tauri/Linux package install"
    return 0
  fi

  if ! command -v apt-get >/dev/null 2>&1; then
    log "No apt-get available; package install skipped"
    return 0
  fi

  log "Installing Linux build, packaging, and Tauri dependencies"
  apt_get update

  local common_packages=(
    build-essential
    curl
    dpkg-dev
    file
    libayatana-appindicator3-dev
    libgtk-3-dev
    libssl-dev
    libxdo-dev
    librsvg2-dev
    patchelf
    pkg-config
    wget
  )

  set +e
  apt_get install -y --no-install-recommends "${common_packages[@]}" libwebkit2gtk-4.1-dev
  local status=$?
  set -e

  if [[ "$status" -ne 0 ]]; then
    log "libwebkit2gtk-4.1-dev unavailable; retrying with libwebkit2gtk-4.0-dev"
    apt_get install -y --no-install-recommends "${common_packages[@]}" libwebkit2gtk-4.0-dev
  fi
}

install_rust() {
  if ! command -v rustup >/dev/null 2>&1; then
    log "Installing rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
  fi

  log "Installing Rust toolchain components from rust-toolchain.toml"
  rustup toolchain install stable --profile minimal --component rustfmt --component clippy
  rustup component add rustfmt clippy
}

install_cargo_tool() {
  local bin="$1"
  local crate="$2"

  if command -v "$bin" >/dev/null 2>&1; then
    log "$bin already installed"
    return 0
  fi

  log "Installing $crate"
  cargo install "$crate" --locked
}

install_rust_tools() {
  install_cargo_tool just just

  if [[ "${CODEX_CLOUD_INSTALL_NEXTEST:-1}" == "1" ]]; then
    install_cargo_tool cargo-nextest cargo-nextest
  else
    log "Skipping cargo-nextest install"
  fi
}

install_node_dependencies() {
  if ! command -v node >/dev/null 2>&1; then
    printf 'error: node is required. Pin Node.js in the Codex Cloud environment package versions.\n' >&2
    exit 1
  fi

  mkdir -p "$PNPM_HOME"

  if command -v corepack >/dev/null 2>&1; then
    log "Activating pnpm through corepack"
    corepack enable
    corepack prepare pnpm@10 --activate
  elif ! command -v pnpm >/dev/null 2>&1; then
    log "Installing pnpm with npm"
    npm install --global pnpm@10
  fi

  log "Installing desktop frontend dependencies"
  pnpm --dir "$ROOT/apps/desktop" install --frozen-lockfile
}

prewarm_rust_dependencies() {
  if [[ "${CODEX_CLOUD_PREWARM_RUST:-1}" != "1" ]]; then
    log "Skipping Rust dependency prewarm"
    return 0
  fi

  log "Fetching Rust dependencies"
  (
    cd "$ROOT"
    cargo fetch --locked
  )
}

prewarm_desktop_dependencies() {
  if [[ "${CODEX_CLOUD_PREWARM_DESKTOP:-0}" != "1" ]]; then
    return 0
  fi

  log "Prewarming desktop typecheck cache"
  (
    cd "$ROOT"
    just web-typecheck
  )
}

main() {
  persist_shell_defaults
  mkdir -p "$CACHE_ROOT" "$KANBAN_CARGO_TARGET_ROOT"

  install_system_packages
  install_rust
  install_rust_tools
  install_node_dependencies
  prewarm_rust_dependencies
  prewarm_desktop_dependencies

  log "Codex Cloud setup complete"
  log "KANBAN_CARGO_TARGET_ROOT=$KANBAN_CARGO_TARGET_ROOT"
}

main "$@"
