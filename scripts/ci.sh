#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/ci.sh <subcommand>

Subcommands:
  frontend  Run the frontend tests (npm test) and frontend build (npm run build)
  rust      Run Rust formatting checks (cargo fmt --check) and tests (cargo test)
  build     Run the Tauri release build without bundling
  all       Run frontend, then rust, then build; stop at the first failure

Options:
  -h, --help  Show this help and exit

The script installs nothing: run npm ci first on a fresh checkout.
EOF
}

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
cd "$repo_root"

frontend() {
  echo "==> Frontend tests"
  npm test
  echo "==> Frontend build"
  npm run build
}

rust() {
  echo "==> Rust formatting"
  ( cd src-tauri && cargo fmt --check )
  echo "==> Rust tests"
  ( cd src-tauri && cargo test )
}

build() {
  echo "==> Tauri release build (no bundle)"
  npm run tauri -- build --no-bundle
}

case "${1-}" in
  frontend)
    frontend
    ;;
  rust)
    rust
    ;;
  build)
    build
    ;;
  all)
    frontend
    rust
    build
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  "")
    usage >&2
    exit 2
    ;;
  *)
    echo "Unknown subcommand: $1" >&2
    usage >&2
    exit 2
    ;;
esac
