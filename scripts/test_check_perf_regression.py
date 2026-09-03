from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


CHECKER = Path(__file__).with_name("check-perf-regression.py").resolve()


def run_gate(
    *,
    baseline_search_ms: float,
    baseline_speedup: str | None,
    current_search_ms: float,
    current_pacman_ms: float | None,
) -> subprocess.CompletedProcess[str]:
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / "benchmarks").mkdir()
        (root / "benchmark_results").mkdir()

        baseline: dict[str, object] = {"search_ms": baseline_search_ms}
        if baseline_speedup is not None:
            baseline["speedup"] = baseline_speedup
        (root / "benchmarks" / "summary.json").write_text(
            json.dumps(baseline), encoding="utf-8"
        )

        results: list[dict[str, object]] = [
            {"command": "OMG (Daemon)", "mean": current_search_ms / 1000}
        ]
        if current_pacman_ms is not None:
            results.append({"command": "pacman", "mean": current_pacman_ms / 1000})
        (root / "benchmark_results" / "search.json").write_text(
            json.dumps({"results": results}), encoding="utf-8"
        )

        environment = os.environ.copy()
        environment.pop("OMG_PERF_THRESHOLD", None)
        return subprocess.run(
            [sys.executable, str(CHECKER)],
            cwd=root,
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )


class PerformanceRegressionGateTests(unittest.TestCase):
    def test_shared_runner_slowdown_passes_when_control_ratio_is_stable(self) -> None:
        result = run_gate(
            baseline_search_ms=6.4,
            baseline_speedup="29.4x",
            current_search_ms=9.4,
            current_pacman_ms=219.0,
        )

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("runner noise", result.stdout)

    def test_search_specific_regression_fails_against_control_ratio(self) -> None:
        result = run_gate(
            baseline_search_ms=6.4,
            baseline_speedup="29.4x",
            current_search_ms=12.0,
            current_pacman_ms=180.0,
        )

        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("PERFORMANCE REGRESSION DETECTED", result.stdout)

    def test_missing_control_falls_back_to_absolute_regression(self) -> None:
        result = run_gate(
            baseline_search_ms=6.4,
            baseline_speedup="29.4x",
            current_search_ms=9.4,
            current_pacman_ms=None,
        )

        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)

    def test_stable_absolute_time_passes_without_a_control(self) -> None:
        result = run_gate(
            baseline_search_ms=6.4,
            baseline_speedup=None,
            current_search_ms=7.0,
            current_pacman_ms=None,
        )

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
