import json
import os
from pathlib import Path
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]


class NetworkScopeTests(unittest.TestCase):
    def test_workflow_requires_namespace_isolation(self):
        workflow = (ROOT / ".github/workflows/qemu-matrix.yml").read_text()
        self.assertEqual(workflow.count("--inventory-isolate-hermetic"), 2)
        runner = (ROOT / "scripts/qemu-inventory.sh").read_text()
        self.assertIn('--isolate-hermetic) isolate_hermetic=true', runner)

    @unittest.skipIf(os.name == "nt", "Linux network namespaces are verified in hosted CI")
    def test_actual_remote_wrapper_has_no_external_interface_and_preserves_user(self):
        source = (ROOT / "scripts/qemu-inventory.sh").read_text()
        line = next(line.strip() for line in source.splitlines() if line.strip().startswith('remote="sudo -n unshare'))
        probe = "import json,os,socket; from pathlib import Path; print(json.dumps(dict(uid=os.getuid(),gid=os.getgid(),interfaces=socket.if_nameindex(),status=Path('/proc/self/status').read_text())))"
        # Build the same quoted inner command that the inventory wraps.
        import shlex
        command = shlex.join([sys.executable, "-c", probe])
        script = 'remote=' + shlex.quote(command) + '\nssh_user=' + shlex.quote(str(os.getuid())) + '\n' + line + '\nbash -c "$remote"'
        result = subprocess.run(["bash", "-euo", "pipefail", "-c", script], capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)
        receipt = json.loads(result.stdout)
        self.assertEqual(receipt["uid"], os.getuid())
        self.assertEqual(receipt["gid"], os.getgid())
        self.assertEqual([name for _, name in receipt["interfaces"]], ["lo"])
        status = dict(line.split(':', 1) for line in receipt["status"].splitlines() if ':' in line)
        self.assertEqual(status["NoNewPrivs"].strip(), "1")
        self.assertEqual(int(status["CapEff"].strip(), 16), 0)


if __name__ == "__main__":
    unittest.main()
