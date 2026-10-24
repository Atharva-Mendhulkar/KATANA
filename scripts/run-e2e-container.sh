#!/usr/bin/env bash
# scripts/run-e2e-container.sh - Automated clean containerized build & test runner (PRD §27, §31)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$ROOT_DIR"

RUNTIME=""
if command -v podman &>/dev/null; then
    RUNTIME="podman"
elif command -v docker &>/dev/null; then
    RUNTIME="docker"
else
    echo "ERROR: Neither podman nor docker found in PATH." >&2
    echo "Please install podman or docker to run containerized tests." >&2
    exit 1
fi

echo "==> Detected container runtime: $RUNTIME"
echo "==> Building Katana clean-room image (katana:test)..."

if ! $RUNTIME build -t katana:test -f Containerfile . ; then
    echo "ERROR: Container build failed. If using Docker, ensure the dockerd service is active." >&2
    exit 1
fi

echo "==> Running in-container doctor diagnostics..."
$RUNTIME run --rm katana:test doctor

echo "==> Running in-container offline replay and demonstration matrix..."
$RUNTIME run --rm katana:test demo all

echo "==> Containerized verification complete: all checks passed."
