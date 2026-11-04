#!/usr/bin/env python3
"""Real terminal regression checks; run with python3 tests/terminal.py target/debug/nosh."""
import errno
import fcntl
import os
from pathlib import Path
import pty
import re
import select
import signal
import struct
import sys
import tempfile
import termios
import time
import unittest

BINARY = str(Path(sys.argv.pop(1) if len(sys.argv) > 1 else 'target/debug/nosh').resolve())
ANSI = re.compile(rb'\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b\[[0-?]*[ -/]*[@-~]')

class Terminal(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='nosh-terminal-')
        self.root = Path(self.temp.name)
        self.config = self.root / 'config'
        self.config.mkdir()
        (self.config / 'config.toml').write_text('onboarding_complete = true\n[prompt]\ntheme = "audit"\nsyntax_highlighting = true\n')
        (self.config / 'themes').mkdir()
        (self.config / 'themes/audit.toml').write_text('[prompt]\nformat = "NOSH> "\n')
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.chdir(self.root)
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 100, 0, 0))
            os.environ['NOSH_CONFIG_DIR'] = str(self.config)
            os.environ['TERM'] = 'xterm-256color'
            os.execv(BINARY, [BINARY, '--norc', '-i'])
        self.transcript = b''
        self.read_until(b'NOSH> ')

    def tearDown(self):
        try:
            foreground = os.tcgetpgrp(self.fd)
            if foreground > 0 and foreground != os.getpgrp(): os.killpg(foreground, signal.SIGKILL)
        except (ProcessLookupError, OSError):
            pass
        try:
            os.kill(self.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        try:
            os.waitpid(self.pid, 0)
        except ChildProcessError:
            pass
        os.close(self.fd)
        self.temp.cleanup()

    def send(self, value):
        os.write(self.fd, value if isinstance(value, bytes) else value.encode())

    def read_until(self, expected, timeout=5, entered=False):
        output = b''
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            ready, _, _ = select.select([self.fd], [], [], max(0, deadline-time.monotonic()))
            if ready:
                try:
                    chunk = os.read(self.fd, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        break
                    raise
                if not chunk:
                    break
                output += chunk
                self.transcript += chunk
                if expected in ANSI.sub(b'', output) and (not entered or b'\r\n__NOSH_DONE__\r\n' in ANSI.sub(b'', output)[:ANSI.sub(b'', output).rfind(expected)]):
                    return ANSI.sub(b'', output)
        self.fail(f'No {expected!r} in terminal output: {ANSI.sub(b"", output)!r}')

    def command(self, command):
        self.send(command + "; printf '\\n__NOSH_DONE__\\n'\r")
        return self.read_until(b'NOSH> ', entered=True)

    def test_editor_and_multiline(self):
        self.command('echo $')
        self.assertIn(b'a#b', self.command('echo a#b'))
        self.send("if true; then\rprintf MULTILINE_OK\rfi; printf '\\n__NOSH_DONE__\\n'\r")
        self.assertIn(b'MULTILINE_OK', self.read_until(b'NOSH> ', entered=True))
        self.assertIn('🍜'.encode(), self.command("printf '%s' '🍜'"))
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack('HHHH', 32, 90, 0, 0))
        os.kill(self.pid, signal.SIGWINCH)
        self.assertIn(b'RESIZED', self.command('printf RESIZED'))
        name = '🍜' * 35
        executable = self.root / name
        executable.write_text('#!/bin/sh\nprintf LONG_NAME_OK\n')
        executable.chmod(0o755)
        self.assertIn(b'LONG_NAME_OK', self.command('./' + name))

    def test_tab_cycles_exact_and_longer_filename(self):
        (self.root / 'xyz.foo').write_text('')
        (self.root / 'xyz.foo.bar').write_text('')
        self.send("printf '<%s>' xyz\t\t; printf '\\n__NOSH_DONE__\\n'\r")
        self.assertIn(b'<xyz.foo.bar>', self.read_until(b'NOSH> ', entered=True))
        self.send("printf '<%s>' xyz\t\t\x1b[Z; printf '\\n__NOSH_DONE__\\n'\r")
        self.assertIn(b'<xyz.foo>', self.read_until(b'NOSH> ', entered=True))
        self.send("printf '<%s>' xyz.foXXXX" + '\x1b[D' * 4 + "\t; printf '\\n__NOSH_DONE__\\n'\r")
        self.assertIn(b'<xyz.foo>', self.read_until(b'NOSH> ', entered=True))

    def test_quoted_paths_and_absolute_commands(self):
        (self.root / 'my report.txt').write_text('')
        self.send("printf '<%s>' my\t; printf '\\n__NOSH_DONE__\\n'\r")
        self.assertIn(b'<my report.txt>', self.read_until(b'NOSH> ', entered=True))
        self.assertIn(b'ABSOLUTE_OK', self.command('/bin/echo ABSOLUTE_OK'))

    def wait_for_job(self):
        # Drain redraw output while waiting: tcsetattr(TCSADRAIN) in the editor
        # may otherwise wait on the fake terminal instead of starting the job.
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if os.tcgetpgrp(self.fd) != self.pid:
                return
            ready, _, _ = select.select([self.fd], [], [], 0.02)
            if ready:
                self.transcript += os.read(self.fd, 65536)
        self.fail('Command never acquired the terminal')

    def test_interrupt_suspend_and_foreground_jobs(self):
        self.send('sleep 60\r')
        self.wait_for_job()
        self.send(b'\x03')
        self.read_until(b'NOSH> ')
        self.assertIn(b'INTERRUPT_OK', self.command('printf INTERRUPT_OK'))
        self.send('sleep 60\r')
        self.wait_for_job()
        self.send(b'\x1a')
        self.read_until(b'NOSH> ')
        self.assertIn(b'sleep', self.command('jobs'))
        self.command('bg')
        self.send('fg\r')
        self.wait_for_job()
        self.send(b'\x03')
        self.read_until(b'NOSH> ')
        self.assertIn(b'JOBS_OK', self.command('printf JOBS_OK'))

if __name__ == '__main__':
    unittest.main()
