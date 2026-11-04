"""Exercise installation and its printed commands without network or system writes."""
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class InstallerTests(unittest.TestCase):
    def test_default_shell_instructions_are_quoted_and_idempotent(self):
        with tempfile.TemporaryDirectory(prefix="nosh-installer-") as temp:
            root = Path(temp)
            tools = root / "tools"
            tools.mkdir()
            binary = root / "nosh"
            binary.write_text("#!/bin/sh\nprintf 'nosh vtest\\n'\n")
            binary.chmod(0o755)
            archive = root / "release.tar.gz"
            with tarfile.open(archive, "w:gz") as tar:
                tar.add(binary, arcname="nosh")
            shells = root / "shells"
            shells.write_text("/bin/bash\n")
            changed = root / "changed-shell"
            def tool(name, body):
                path = tools / name
                path.write_text(f"#!{sys.executable}\n" + body)
                path.chmod(0o755)
            tool("uname", "import sys\nprint('Linux' if sys.argv[1] == '-s' else 'x86_64')\n")
            tool("curl", "import os,sys,shutil\n"
                 "if '-o' in sys.argv:\n"
                 " shutil.copyfile(os.environ['TEST_ARCHIVE'], sys.argv[sys.argv.index('-o')+1])\n"
                 "else: print('{\"tag_name\":\"vtest\"}')\n")
            # These stubs prohibit any actual privileged writes or shell changes.
            tool("sudo", "import os,sys\n"
                 "assert sys.argv[1:] == ['tee', '-a', '/etc/shells'], sys.argv\n"
                 "with open(os.environ['TEST_SHELLS'], 'a') as f: f.write(sys.stdin.read())\n")
            tool("grep", "import os,subprocess,sys\n"
                 "args=[os.environ['TEST_SHELLS'] if a == '/etc/shells' else a for a in sys.argv[1:]]\n"
                 "sys.exit(subprocess.call(['/usr/bin/grep', *args]))\n")
            tool("chsh", "import os,sys\n"
                 "assert sys.argv[1] == '-s' and len(sys.argv) == 3, sys.argv\n"
                 "with open(os.environ['TEST_CHANGED'], 'w') as f: f.write(sys.argv[2])\n")
            install = root / "install dir's bin"
            env = dict(os.environ, HOME=str(root), NOSH_INSTALL_DIR=str(install),
                       PATH=str(tools) + os.pathsep + os.environ['PATH'],
                       TEST_ARCHIVE=str(archive), TEST_SHELLS=str(shells),
                       TEST_CHANGED=str(changed))
            result = subprocess.run(['sh', str(ROOT / 'install.sh')], env=env,
                                    capture_output=True, text=True, check=True)
            self.assertTrue((install / 'nosh').is_file())
            self.assertFalse(changed.exists(), 'installation must not change the default shell')
            self.assertEqual(shells.read_text(), '/bin/bash\n')
            instructions = result.stdout.split('run these commands in Bash or Zsh:\n', 1)[1]
            instructions = instructions.split('Documentation:', 1)[0].strip()
            self.assertIn('grep -Fxq', instructions)
            # Execute the exact printed instructions twice, including a quoted path.
            for _ in range(2):
                subprocess.run(['sh', '-c', instructions], env=env, check=True,
                               capture_output=True, text=True)
            expected = str((install / 'nosh').resolve())
            self.assertEqual(changed.read_text(), expected)
            self.assertEqual(shells.read_text().splitlines(), ['/bin/bash', expected])


if __name__ == '__main__':
    unittest.main()
