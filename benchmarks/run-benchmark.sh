#!/bin/bash
# Benchmark script comparing nosh vs zsh+spaceship startup time

set -e

echo "============================================"
echo "Shell Startup Time Benchmark"
echo "============================================"
echo ""
echo "Environment:"
echo "  - zsh $(zsh --version | head -1)"
echo "  - spaceship-prompt (latest)"
echo "  - nosh (built from source)"
echo "  - Directory: /test-project (git repo with Cargo.toml)"
echo ""

# Warmup runs (prime caches, etc.)
echo "Warming up..."
for i in {1..5}; do
    zsh -i -c 'exit' 2>/dev/null
    nosh -c 'exit' 2>/dev/null || true
done

echo ""
echo "============================================"
echo "Benchmark Results (100 runs each)"
echo "============================================"
echo ""

# Run hyperfine benchmark
# -i: ignore non-zero exit codes
# -w: warmup runs
# -m: minimum runs
hyperfine \
    --warmup 5 \
    --min-runs 100 \
    --export-json /tmp/benchmark.json \
    --export-markdown /tmp/benchmark.md \
    'zsh -i -c exit' \
    'nosh -c exit' \
    2>&1

echo ""
echo "============================================"
echo "Markdown Output"
echo "============================================"
cat /tmp/benchmark.md

echo ""
echo "============================================"
echo "JSON Output (for website)"
echo "============================================"
cat /tmp/benchmark.json
