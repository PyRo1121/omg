"""Execute the production process-status gate against unsafe QEMU identities."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
BASH = 'C:/Program Files/Git/bin/bash.exe' if os.name == 'nt' else 'bash'


class QemuProcessIsolationTests(unittest.TestCase):
    def test_process_gate_rejects_root_capabilities_and_missing_restrictions(self):
        source = (ROOT / 'scripts/benchmark-qemu.sh').read_text()
        script = source.split('qemu_pid=$(<qemu.pid)', 1)[1].split('opts=(', 1)[0]
        script = script[script.index("awk '"):script.index("printf 'QEMU isolation verified")]
        script = script.replace('"/proc/$qemu_pid/status"', '"$STATUS_FILE"')
        safe = ('Uid:\t65534\t65534\t65534\t65534\n'
                'Gid:\t65534\t65534\t65534\t65534\n'
                'CapEff:\t0000000000000000\nNoNewPrivs:\t1\nSeccomp:\t2\n')
        cases = [safe, '', safe.replace('65534', '0', 1),
                 safe.replace('CapEff:\t0000000000000000', 'CapEff:\t0000000000000001'),
                 safe.replace('NoNewPrivs:\t1', 'NoNewPrivs:\t0'),
                 safe.replace('Seccomp:\t2', 'Seccomp:\t0'),
                 safe.replace('Gid:\t65534', 'Gid:\t0')]
        for index, status in enumerate(cases):
            with self.subTest(index=index), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'status'
                path.write_text(status)
                result = subprocess.run([BASH, '-c', script],
                                        env=dict(os.environ, STATUS_FILE=str(path)),
                                        capture_output=True, text=True, timeout=10)
                self.assertEqual(result.returncode == 0, index == 0, result.stderr)


if __name__ == '__main__':
    unittest.main()
