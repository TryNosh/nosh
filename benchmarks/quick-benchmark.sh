#!/bin/bash
# Quick local benchmark (requires hyperfine: brew install hyperfine)

set -e

cd "$(dirname "$0")/.."

echo "Quick Shell Startup Benchmark"
echo "=============================="
echo ""

# Check if hyperfine is installed
if ! command -v hyperfine &> /dev/null; then
    echo "hyperfine not found. Install with: brew install hyperfine"
    exit 1
fi

# Build nosh in release mode
echo "Building nosh (release)..."
cargo build --release --quiet

NOSH="./target/release/nosh"

echo ""
echo "Running benchmark in current directory..."
echo "  Directory: $(pwd)"
echo "  Git: $(git branch --show-current 2>/dev/null || echo 'not a git repo')"
echo ""

# Quick benchmark (fewer runs for speed)
hyperfine \
    --warmup 3 \
    --min-runs 50 \
    --export-markdown /tmp/nosh-bench.md \
    'zsh -i -c exit' \
    "$NOSH -c exit"

echo ""
echo "Results saved to /tmp/nosh-bench.md"
