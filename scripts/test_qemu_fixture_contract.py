"""Verify real shell command lookup for the missing-Cargo guest fixture."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


@unittest.skipIf(os.name == 'nt', 'Guest POSIX command lookup runs in hosted Linux CI')
class CargoFixtureTests(unittest.TestCase):
    def test_present_cargo_is_setup_failure_and_absent_cargo_is_valid(self):
        source = (Path(__file__).resolve().parent / 'benchmark-qemu.sh').read_text()
        begin = source.index('  if command -v cargo > evidence/rust-toolchain.txt; then')
        end = source.index('\n  fi', begin) + len('\n  fi')
        script = source[begin:end]
        bash = shutil.which('bash')
        self.assertIsNotNone(bash)
        for present in (False, True):
            with self.subTest(present=present), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / 'evidence').mkdir()
                (root / 'bin').mkdir()
                if present:
                    cargo = root / 'bin/cargo'
                    cargo.write_text('#!/bin/sh\nexit 0\n')
                    cargo.chmod(0o755)
                env = dict(os.environ, PATH=str(root / 'bin'))
                result = subprocess.run([bash, '-c', script], cwd=root, env=env,
                                        capture_output=True, text=True, timeout=10)
                self.assertEqual(result.returncode, 120 if present else 0, result.stderr)
                evidence = (root / 'evidence/rust-toolchain.txt').read_text()
                self.assertEqual(bool(evidence.strip()), present)


if __name__ == '__main__':
    unittest.main()
