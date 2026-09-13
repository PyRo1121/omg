import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("ci_smoke_report", Path(__file__).with_name("ci-smoke-report.py"))
REPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPORT)


class ReportingTests(unittest.TestCase):
    def test_secure_config_and_no_secret_export(self):
        with tempfile.TemporaryDirectory() as directory:
            export = Path(directory) / "env"
            with patch.dict(os.environ, {"RUNNER_TEMP": directory, "GITHUB_ENV": str(export), "OMG_SMOKE_SENTRY_DSN": "https://abc@o1.ingest.sentry.io/123"}):
                REPORT.configure()
            config = Path(directory) / "omg-smoke-config/sentry.json"
            self.assertEqual(json.loads(config.read_text())["dsn"], "https://abc@o1.ingest.sentry.io/123")
            self.assertNotIn("https://", export.read_text())
            if os.name != "nt":
                self.assertEqual(config.stat().st_mode & 0o777, 0o600)

    def test_invalid_secret_does_not_write_config(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.dict(os.environ, {"RUNNER_TEMP": directory, "OMG_SMOKE_SENTRY_DSN": "bad\nsecret"}):
                with self.assertRaises(ValueError):
                    REPORT.configure()
            self.assertFalse((Path(directory) / "omg-smoke-config").exists())

    def test_failure_projection_and_transport_failure_preserve_status(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(REPORT.subprocess, "run") as run:
                run.return_value.returncode = 1
                run.return_value.stdout = "Sentry rejected report with HTTP 429\n"
                run.return_value.stderr = ""
                self.assertEqual(REPORT.status("arch", "qemu-arch-build", "failure", Path(directory)), 0)
            result = json.loads((Path(directory) / "results.json").read_text())
            self.assertEqual(result, [{"case_id": "qemu-arch-build", "distro": "arch", "result": "HARNESS_ERROR", "exit_code": 1, "elapsed_seconds": 0}])
            self.assertIn("429", (Path(directory) / "reporting.log").read_text())

    def test_success_and_skipped_never_send(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(REPORT.subprocess, "run") as run:
            for state in ("success", "skipped"):
                self.assertEqual(REPORT.status("arch", "qemu-arch-build", state, Path(directory)), 0)
            run.assert_not_called()

    def test_reject_untrusted_case_id(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(REPORT.subprocess, "run") as run:
            with self.assertRaises(ValueError):
                REPORT.status("arch", "bad\nsecret", "failure", Path(directory))
            run.assert_not_called()

    def test_missing_secret_is_visible_and_writes_nothing(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.dict(os.environ, {"RUNNER_TEMP": directory, "OMG_SMOKE_SENTRY_DSN": ""}):
                self.assertEqual(REPORT.configure(), 0)
            self.assertEqual(list(Path(directory).iterdir()), [])

    def test_existing_config_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "omg-smoke-config"
            root.mkdir()
            config = root / "sentry.json"
            config.write_text("original")
            with patch.dict(os.environ, {"RUNNER_TEMP": directory, "OMG_SMOKE_SENTRY_DSN": "https://abc@o1.ingest.sentry.io/123"}):
                with self.assertRaises(FileExistsError):
                    REPORT.configure()
            self.assertEqual(config.read_text(), "original")

    def test_verify_accepts_successful_delivery_receipts(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory)
            run = evidence / "run-safe"
            run.mkdir()
            (run / "reporting-status.json").write_text(
                json.dumps({"exit_code": 0}) + "\n", encoding="utf-8"
            )
            self.assertEqual(REPORT.verify(evidence), 0)

    def test_verify_rejects_failed_or_missing_delivery_receipts(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory)
            self.assertNotEqual(REPORT.verify(evidence), 0)
            run = evidence / "run-failed"
            run.mkdir()
            (run / "reporting-status.json").write_text(
                json.dumps({"exit_code": 1}) + "\n", encoding="utf-8"
            )
            self.assertNotEqual(REPORT.verify(evidence), 0)

    def test_missing_bash_preserves_failure_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(REPORT.subprocess, "run", side_effect=FileNotFoundError):
                self.assertEqual(REPORT.status("ubuntu", "qemu-ubuntu-preflight", "cancelled", Path(directory)), 0)
            self.assertEqual(json.loads((Path(directory) / "results.json").read_text())[0]["exit_code"], 130)

    def test_verify_requires_a_receipt_for_every_exported_run(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory)
            good = evidence / "run-good"
            good.mkdir()
            (good / "reporting-status.json").write_text('{"exit_code": 0}')
            (evidence / "run-missing").mkdir()
            self.assertNotEqual(REPORT.verify(evidence), 0)


if __name__ == "__main__":
    unittest.main()
