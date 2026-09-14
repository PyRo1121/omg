from __future__ import annotations

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


GATE = Path(__file__).with_name("require-workflow-success.sh").resolve()
FAKE_GH = """gh() {
if [[ "$1 $2" == "run list" ]]; then
  # A release must not select an older success or PR-controlled run.
  [[ "$*" != *"--status success"* && "$*" == *"--event push"* && "$*" == *"--limit 1"* ]] || return 2
  case "$FAKE_GH_STATE" in
    success) printf '101\\tcompleted\\tsuccess\\n' ;;
    pending|failed) printf '202\\tin_progress\\t\\n' ;;
    newer_failure) printf '303\\tcompleted\\tfailure\\n' ;;
    cancelled) printf '303\\tcompleted\\tcancelled\\n' ;;
    skipped) printf '303\\tcompleted\\tskipped\\n' ;;
  esac
  return 0
fi
if [[ "$1 $2" == "run watch" ]]; then
  [[ "$FAKE_GH_STATE" == "pending" ]]
  return
fi
return 2
}
export -f gh
bash "$GATE" ci.yml abc123 CI
"""


def run_gate(state: str) -> subprocess.CompletedProcess[str]:
    with tempfile.TemporaryDirectory():
        environment = os.environ.copy()
        environment["GATE"] = str(GATE)
        environment["GITHUB_REPOSITORY"] = "owner/repository"
        environment["FAKE_GH_STATE"] = state
        return subprocess.run(
            [os.environ.get('OMG_TEST_BASH', 'C:/Program Files/Git/bin/bash.exe' if os.name == 'nt' else 'bash'), '-c', FAKE_GH],
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )


class WorkflowSuccessGateTests(unittest.TestCase):
    def test_accepts_an_existing_successful_run(self) -> None:
        result = run_gate("success")

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("run 101", result.stdout)

    def test_waits_for_an_in_progress_run(self) -> None:
        result = run_gate("pending")

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Waiting for CI run 202", result.stdout)

    def test_rejects_a_failed_in_progress_run(self) -> None:
        result = run_gate("failed")

        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("run 202 failed", result.stderr)

    def test_rejects_missing_evidence(self) -> None:
        result = run_gate("missing")

        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("No push-triggered CI run", result.stderr)

    def test_latest_failed_cancelled_or_skipped_run_blocks_older_success(self):
        for state in ('newer_failure', 'cancelled', 'skipped'):
            with self.subTest(state=state):
                result = run_gate(state)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertIn('Latest CI run 303 did not succeed', result.stderr)


if __name__ == "__main__":
    unittest.main()
