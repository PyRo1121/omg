"""Regression checks for unbiased Hyperfine evidence admission."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Protocol, cast


class Recorder(Protocol):
    def validate_results(self, source: Path) -> list[str]: ...


def load_recorder() -> Recorder:
    path = Path(__file__).resolve().parents[1] / "scripts/record-benchmark-run.py"
    spec = importlib.util.spec_from_file_location("benchmark_recorder", path)
    if spec is None or spec.loader is None:
        raise RuntimeError("Cannot load benchmark recorder")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return cast(Recorder, module)


def measurement(name: str, seconds: float) -> dict[str, object]:
    return {
        "command": name,
        "mean": seconds,
        "median": seconds,
        "min": seconds,
        "max": seconds,
        "stddev": 0.0,
        "user": 0.0,
        "system": 0.0,
        "times": [seconds, seconds],
        "exit_codes": [0, 0],
    }


class BenchmarkAdmissionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory(prefix="benchmark-record-test-")
        self.addCleanup(self.directory.cleanup)
        self.source = Path(self.directory.name)
        self.recorder = load_recorder()

    def write_results(self, *results: dict[str, object]) -> None:
        (self.source / "search.json").write_text(
            json.dumps({"results": results}), encoding="utf-8"
        )

    def test_retains_slower_results(self) -> None:
        self.write_results(measurement("OMG (Daemon)", 0.3), measurement("pacman", 0.1))
        self.assertEqual(self.recorder.validate_results(self.source), [])

    def test_retains_parity(self) -> None:
        self.write_results(measurement("OMG (Daemon)", 0.1), measurement("pacman", 0.1))
        self.assertEqual(self.recorder.validate_results(self.source), [])

    def test_no_arbitrary_time_or_cpu_floor(self) -> None:
        self.write_results(
            measurement("OMG (Daemon)", 0.00001), measurement("pacman", 0.00002)
        )
        self.assertEqual(self.recorder.validate_results(self.source), [])

    def test_accepts_actual_driver_command_label(self) -> None:
        self.write_results(measurement("OMG", 0.1), measurement("pacman", 0.2))
        self.assertEqual(self.recorder.validate_results(self.source), [])

    def test_rejects_failed_sample(self) -> None:
        result = measurement("OMG (Daemon)", 0.1)
        result["exit_codes"] = [0, 1]
        self.write_results(result)
        self.assertTrue(self.recorder.validate_results(self.source))

    def test_rejects_missing_exit_receipt(self) -> None:
        result = measurement("OMG (Daemon)", 0.1)
        result["exit_codes"] = [0]
        self.write_results(result)
        self.assertTrue(self.recorder.validate_results(self.source))

    def test_rejects_nonfinite_sample(self) -> None:
        result = measurement("OMG (Daemon)", 0.1)
        result["times"] = [0.1, float("nan")]
        self.write_results(result)
        self.assertTrue(self.recorder.validate_results(self.source))

    def test_rejects_fabricated_summary(self) -> None:
        result = measurement("OMG (Daemon)", 0.1)
        result["times"] = [0.2, 0.2]
        self.write_results(result)
        self.assertTrue(self.recorder.validate_results(self.source))

    def test_rejects_boolean_exit_code(self) -> None:
        result = measurement("OMG (Daemon)", 0.1)
        result["exit_codes"] = [False, False]
        self.write_results(result)
        self.assertTrue(self.recorder.validate_results(self.source))


if __name__ == "__main__":
    unittest.main()
