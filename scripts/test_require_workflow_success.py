from __future__ import annotations

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


GATE = Path(__file__).with_name("require-workflow-success.sh").resolve()
FAKE_GH = """#!/usr/bin/env bash
set -euo pipefail
if [[ "$1 $2" == "run list" ]]; then
  if [[ "$*" == *"--status success"* ]]; then
    [[ "$FAKE_GH_STATE" == "success" ]] && echo 101
  elif [[ "$FAKE_GH_STATE" == "pending" || "$FAKE_GH_STATE" == "failed" ]]; then
    echo 202
  fi
  exit 0
fi
if [[ "$1 $2" == "run watch" ]]; then
  [[ "$FAKE_GH_STATE" == "pending" ]]
  exit
fi
exit 2
"""


def run_gate(state: str) -> subprocess.CompletedProcess[str]:
    with tempfile.TemporaryDirectory() as directory:
        bin_directory = Path(directory)
        gh = bin_directory / "gh"
        gh.write_text(FAKE_GH, encoding="utf-8")
        gh.chmod(0o755)

        environment = os.environ.copy()
        environment["PATH"] = f"{bin_directory}:{environment['PATH']}"
        environment["GITHUB_REPOSITORY"] = "owner/repository"
        environment["FAKE_GH_STATE"] = state
        return subprocess.run(
            [str(GATE), "ci.yml", "abc123", "CI"],
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
        self.assertIn("No successful or in-progress CI run", result.stderr)


if __name__ == "__main__":
    unittest.main()
