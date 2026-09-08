"""Regression checks for unbiased Hyperfine evidence admission."""

from __future__ import annotations

import importlib.util
import json
import os
import shutil
import subprocess
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

    def test_missing_hyperfine_never_runs_substitute_timer(self) -> None:
        tools = self.source / "tools"
        tools.mkdir()
        for name in ("dirname", "mkdir"):
            executable = shutil.which(name)
            if executable is None:
                raise RuntimeError(f"Missing fixture prerequisite: {name}")
            (tools / name).symlink_to(executable)
        fallback = self.source / "benchmark.sh"
        fallback.write_text("#!/bin/sh\nexit 77\n", encoding="utf-8")
        fallback.chmod(0o700)
        environment = {
            **os.environ,
            "HOME": str(self.source / "home"),
            "PATH": str(tools),
            "OMG_BENCH_EXPORT_DIR": str(self.source / "output"),
        }
        script = Path(__file__).resolve().parents[1] / "benchmark-hyperfine.sh"
        result = subprocess.run(
            ["/bin/bash", str(script), "--fast"],
            cwd=self.source,
            env=environment,
            capture_output=True,
            text=True,
            check=False,
            timeout=10,
        )
        self.assertEqual(result.returncode, 3, result.stdout + result.stderr)
        self.assertIn("no substitute timer", result.stderr)

    def test_guest_mode_refuses_implicit_source_build(self) -> None:
        tools = self.source / "tools"
        tools.mkdir()
        for name in ("dirname", "mkdir"):
            executable = shutil.which(name)
            if executable is None:
                raise RuntimeError(f"Missing fixture prerequisite: {name}")
            (tools / name).symlink_to(executable)
        for name in ("cargo", "hyperfine"):
            stub = tools / name
            stub.write_text("#!/bin/sh\nexit 77\n", encoding="utf-8")
            stub.chmod(0o700)
        environment = {
            **os.environ,
            "HOME": str(self.source / "home"),
            "PATH": str(tools),
            "OMG_BENCH_BINARY": "",
            "OMG_BENCH_EXPORT_DIR": str(self.source / "output"),
        }
        script = Path(__file__).resolve().parents[1] / "benchmark-hyperfine.sh"
        result = subprocess.run(
            ["/bin/bash", str(script), "--guest"],
            cwd=self.source,
            env=environment,
            capture_output=True,
            text=True,
            check=False,
            timeout=10,
        )
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("prebuilt binary", result.stderr)

    def test_full_run_does_not_hide_update_benchmark_failure(self) -> None:
        tools = self.source / "tools"
        tools.mkdir()
        for name in (
            "dirname",
            "mkdir",
            "mktemp",
            "chmod",
            "rm",
            "seq",
            "grep",
            "cat",
            "wc",
            "tr",
            "python3",
            "tail",
        ):
            executable = shutil.which(name)
            if executable is None:
                raise RuntimeError(f"Missing fixture prerequisite: {name}")
            (tools / name).symlink_to(executable)
        stubs = {
            "omg": '#!/bin/bash\ncase "$1" in ec) echo 1;; *) echo firefox;; esac\n',
            "omgd": "#!/bin/bash\nexec /usr/bin/sleep 60\n",
            "hyperfine": "#!/bin/bash\nexit 0\n",
        }
        for name, content in stubs.items():
            path = tools / name
            path.write_text(content, encoding="utf-8")
            path.chmod(0o700)
        environment = {
            **os.environ,
            "HOME": str(self.source / "home"),
            "PATH": str(tools),
            "OMG_BENCH_BINARY": str(tools / "omg"),
            "OMG_BENCH_EXPORT_DIR": str(self.source / "output"),
            "OMG_BENCH_SOURCE_CACHE": str(self.source / "absent-cache"),
            "OMG_BENCH_SKIP_UPDATE": "0",
            "OMG_BENCH_SKIP_RECORD": "1",
        }
        script = Path(__file__).resolve().parents[1] / "benchmark-hyperfine.sh"
        result = subprocess.run(
            ["/bin/bash", str(script)],
            cwd=self.source,
            env=environment,
            capture_output=True,
            text=True,
            check=False,
            timeout=10,
        )
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        # Root execution is also deliberately refused before update discovery.
        self.assertTrue(
            "Missing benchmark fixture" in result.stderr
            or "Run the update benchmark as a regular user" in result.stderr,
            result.stdout + result.stderr,
        )
        self.assertNotIn("Benchmarks Complete!", result.stdout)

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
