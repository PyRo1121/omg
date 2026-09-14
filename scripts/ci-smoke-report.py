#!/usr/bin/env python3
"""Configure CI telemetry and report failures before the smoke harness starts.

Run configure with the DSN in the environment, never in shell source. Run status
from an always-running failure step. Evidence directories must not contain config.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess


def configure():
    dsn = os.environ.get("OMG_SMOKE_SENTRY_DSN", "")
    if not dsn:
        print("::notice::Sentry reporting disabled: OMG_SMOKE_SENTRY_DSN is unavailable")
        return 0
    if not re.fullmatch(r"https://[A-Za-z0-9]+@[a-z0-9.-]+\.sentry\.io/[0-9]+", dsn):
        raise ValueError("Sentry DSN has an unexpected shape")
    root = Path(os.environ["RUNNER_TEMP"]) / "omg-smoke-config"
    root.mkdir(mode=0o700, parents=True, exist_ok=True)
    config = root / "sentry.json"
    # Exclusive creation refuses symlinks and unexpected pre-existing files.
    descriptor = os.open(config, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
        json.dump({"dsn": dsn}, stream)
    with open(os.environ["GITHUB_ENV"], "a", encoding="utf-8") as stream:
        stream.write(f"OMG_SMOKE_SENTRY_CONFIG={config}\n")
    print("Sentry reporting enabled")
    return 0


def status(distro, case_id, state, evidence):
    if distro not in ("arch", "debian", "ubuntu", "fedora", "macos"):
        raise ValueError("unsupported distro")
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,127}", case_id):
        raise ValueError("invalid case identifier")
    if state not in ("success", "skipped", "failure", "cancelled"):
        raise ValueError("unsupported job status")
    if state in ("success", "skipped"):
        print(f"Sentry report not needed: job {state}")
        return 0
    evidence.mkdir(parents=True, exist_ok=True)
    results = evidence / "results.json"
    results.write_text(json.dumps([{
        "case_id": case_id, "distro": distro, "result": "HARNESS_ERROR",
        "exit_code": 130 if state == "cancelled" else 1, "elapsed_seconds": 0,
    }]) + "\n", encoding="utf-8")
    reporter = Path(__file__).with_name("report-smoke-sentry.sh")
    try:
        completed = subprocess.run(["bash", str(reporter), str(results)],
                                   capture_output=True, text=True, timeout=15, check=False)
        output = completed.stdout + completed.stderr
        failed = completed.returncode != 0
    except (OSError, subprocess.TimeoutExpired) as error:
        output = f"Sentry reporting unavailable: {type(error).__name__}\n"
        failed = True
    (evidence / "reporting.log").write_text(output, encoding="utf-8")
    print(output, end="")
    if failed:
        print("::warning::Sentry delivery failed; original job failure and local evidence are preserved")
    return 0


def verify(evidence_root):
    if not evidence_root.is_dir():
        print("::error::Sentry delivery evidence directory is missing")
        return 2
    runs = sorted(evidence_root.glob("run-*"))
    if not runs:
        print("::error::Sentry delivery receipt is missing")
        return 2
    receipts = []
    for run in runs:
        if run.is_symlink() or not run.is_dir():
            print("::error::Invalid Sentry delivery run directory")
            return 2
        receipts.append(run / "reporting-status.json")
    failures = []
    for receipt in receipts:
        if receipt.is_symlink() or not receipt.is_file() or receipt.stat().st_size > 4096:
            print(f"::error::Invalid Sentry delivery receipt: {receipt.name}")
            return 2
        payload = json.loads(receipt.read_text(encoding="utf-8"))
        exit_code = payload.get("exit_code") if isinstance(payload, dict) else None
        if isinstance(exit_code, bool) or not isinstance(exit_code, int) or not 0 <= exit_code <= 255:
            print(f"::error::Invalid Sentry delivery status: {receipt.name}")
            return 2
        if exit_code != 0:
            failures.append((receipt.parent.name, exit_code))
    if failures:
        detail = ", ".join(f"{run}=exit-{code}" for run, code in failures)
        print(f"::error::Sentry delivery failed: {detail}")
        return 1
    print(f"Verified {len(receipts)} Sentry delivery receipt(s)")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("configure")
    report = commands.add_parser("status")
    report.add_argument("--distro", required=True)
    report.add_argument("--case-id", required=True)
    report.add_argument("--status", required=True)
    report.add_argument("--evidence-dir", type=Path, required=True)
    verify_parser = commands.add_parser("verify")
    verify_parser.add_argument("--evidence-root", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "configure":
            return configure()
        if args.command == "verify":
            return verify(args.evidence_root)
        return status(args.distro, args.case_id, args.status, args.evidence_dir)
    except (ValueError, KeyError, OSError):
        print("::error::Invalid Sentry configuration or reporting input; check configuration and paths")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
