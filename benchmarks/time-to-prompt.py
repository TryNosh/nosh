#!/usr/bin/env python3
"""
Measure time-to-first-prompt for shells.

This measures the real user experience: from invoking the shell binary
to when the prompt is fully rendered and ready for input.
"""

import subprocess
import time
import os
import sys
import pty
import select
import statistics

# Prompt patterns to detect (when prompt is ready)
# Note: UTF-8 encoded characters
# ❯ = \xe2\x9d\xaf (U+276F)
# ➜ = \xe2\x9e\x9c (U+279C) - spaceship default
PROMPT_PATTERNS = {
    'zsh': [b'\xe2\x9e\x9c', b'%', b'>', b'$', b'\xe2\x9d\xaf'],  # spaceship uses ➜
    'nosh': [b'\xe2\x9d\xaf', b'>'],  # nosh uses ❯ by default
}

def time_to_prompt(cmd: list[str], shell_type: str, timeout: float = 10.0) -> float | None:
    """
    Measure time from shell start to first prompt appearance.

    Returns time in seconds, or None if timeout.
    """
    patterns = PROMPT_PATTERNS.get(shell_type, [b'>'])

    # Create a pseudo-terminal
    master_fd, slave_fd = pty.openpty()

    start_time = time.perf_counter()

    try:
        # Start the shell process
        proc = subprocess.Popen(
            cmd,
            stdin=slave_fd,
            stdout=slave_fd,
            stderr=slave_fd,
            close_fds=True,
            env={**os.environ, 'TERM': 'xterm-256color'},
        )

        os.close(slave_fd)

        output = b''
        deadline = start_time + timeout

        while time.perf_counter() < deadline:
            # Wait for data with short timeout
            ready, _, _ = select.select([master_fd], [], [], 0.01)

            if ready:
                try:
                    chunk = os.read(master_fd, 4096)
                    if chunk:
                        output += chunk

                        # Check if any prompt pattern appears
                        for pattern in patterns:
                            if pattern in output:
                                elapsed = time.perf_counter() - start_time
                                proc.terminate()
                                return elapsed
                except OSError:
                    break

            # Check if process died
            if proc.poll() is not None:
                break

        proc.terminate()
        return None

    finally:
        try:
            os.close(master_fd)
        except OSError:
            pass


def benchmark(cmd: list[str], shell_type: str, runs: int = 20, warmup: int = 3) -> dict:
    """Run multiple measurements and compute statistics."""

    # Warmup runs
    for _ in range(warmup):
        time_to_prompt(cmd, shell_type)

    # Actual measurements
    times = []
    for i in range(runs):
        t = time_to_prompt(cmd, shell_type)
        if t is not None:
            times.append(t)
        sys.stdout.write('.')
        sys.stdout.flush()

    print()

    if not times:
        return {'error': 'All runs timed out'}

    return {
        'mean': statistics.mean(times),
        'median': statistics.median(times),
        'stdev': statistics.stdev(times) if len(times) > 1 else 0,
        'min': min(times),
        'max': max(times),
        'runs': len(times),
    }


def format_ms(seconds: float) -> str:
    """Format seconds as milliseconds."""
    return f"{seconds * 1000:.1f}ms"


def main():
    import json

    print("Time-to-First-Prompt Benchmark")
    print("=" * 50)
    print()

    # Find nosh binary - check common locations
    nosh_bin = None
    for path in ['/usr/local/bin/nosh', './target/release/nosh']:
        if os.path.exists(path):
            nosh_bin = path
            break

    if nosh_bin is None:
        # Try to build it
        script_dir = os.path.dirname(os.path.abspath(__file__))
        project_dir = os.path.dirname(script_dir)
        nosh_bin = os.path.join(project_dir, 'target', 'release', 'nosh')
        if not os.path.exists(nosh_bin):
            print("Building nosh (release)...")
            subprocess.run(['cargo', 'build', '--release', '--quiet'], cwd=project_dir, check=True)

    # Change to test directory if it exists (Docker), otherwise use project dir
    if os.path.exists('/test-project'):
        os.chdir('/test-project')
    else:
        script_dir = os.path.dirname(os.path.abspath(__file__))
        project_dir = os.path.dirname(script_dir)
        os.chdir(project_dir)

    print(f"Directory: {os.getcwd()}")
    print(f"Git branch: {subprocess.getoutput('git branch --show-current')}")
    print(f"nosh binary: {nosh_bin}")
    print()

    shells = [
        ('zsh + spaceship', ['zsh', '-i'], 'zsh'),
        ('nosh', [nosh_bin], 'nosh'),
    ]

    results = {}
    runs = 50  # More runs for better statistics

    for name, cmd, shell_type in shells:
        print(f"Benchmarking {name} ({runs} runs)...", end=' ')
        results[name] = benchmark(cmd, shell_type, runs=runs, warmup=5)

    print()
    print("Results")
    print("=" * 50)
    print()

    for name, stats in results.items():
        if 'error' in stats:
            print(f"{name}: {stats['error']}")
        else:
            print(f"{name}:")
            print(f"  Mean:   {format_ms(stats['mean'])}")
            print(f"  Median: {format_ms(stats['median'])}")
            print(f"  Min:    {format_ms(stats['min'])}")
            print(f"  Max:    {format_ms(stats['max'])}")
            print(f"  StdDev: {format_ms(stats['stdev'])}")
            print()

    # Calculate speedup
    zsh_stats = results.get('zsh + spaceship', {})
    nosh_stats = results.get('nosh', {})

    if 'error' not in zsh_stats and 'error' not in nosh_stats:
        zsh_mean = zsh_stats['mean']
        nosh_mean = nosh_stats['mean']
        speedup = zsh_mean / nosh_mean

        print("=" * 50)
        print(f"nosh is {speedup:.1f}x faster than zsh + spaceship")
        print(f"zsh + spaceship: {format_ms(zsh_mean)}")
        print(f"nosh:            {format_ms(nosh_mean)}")
        print(f"Time saved:      {format_ms(zsh_mean - nosh_mean)} per prompt")
        print()

        # Output JSON for website
        output = {
            'zsh_spaceship_ms': round(zsh_mean * 1000, 1),
            'nosh_ms': round(nosh_mean * 1000, 1),
            'speedup': round(speedup, 1),
            'time_saved_ms': round((zsh_mean - nosh_mean) * 1000, 1),
        }
        print("JSON for website:")
        print(json.dumps(output, indent=2))


if __name__ == '__main__':
    main()
