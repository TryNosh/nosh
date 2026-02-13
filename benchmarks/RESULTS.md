# Benchmark Results

## Time-to-First-Prompt Comparison

Measured in Docker container (Debian Bookworm) with:
- zsh 5.9 + spaceship-prompt (latest)
- nosh (built from source)
- Test directory: git repo with Cargo.toml (realistic project setup)

### Results (50 runs each)

| Shell | Mean | Median | Min | Max |
|-------|------|--------|-----|-----|
| zsh + spaceship | 150-160ms | ~152ms | ~142ms | ~235ms |
| nosh | 1.7ms | 1.7ms | 1.4ms | ~2.8ms |

### Summary

**nosh is ~90x faster than zsh + spaceship**

- zsh + spaceship: ~155ms
- nosh: ~1.7ms
- Time saved: ~153ms per prompt

### Why the difference?

1. **Native Rust** - nosh is compiled, not interpreted
2. **Async plugin execution** - plugins run in parallel with 100ms soft timeout
3. **Internal context detection** - git/language info via library, not shell commands
4. **No shell startup overhead** - nosh doesn't source .zshrc, .oh-my-zsh, etc.

### Reproducing

```bash
# Build benchmark container
docker build -t nosh-benchmark -f benchmarks/Dockerfile .

# Run benchmark
docker run --rm nosh-benchmark
```

### JSON for Website

```json
{
  "zsh_spaceship_ms": 155,
  "nosh_ms": 1.7,
  "speedup": 90,
  "time_saved_ms": 153
}
```
