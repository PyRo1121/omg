#!/usr/bin/env python3
"""Archive a hyperfine run into benchmarks/records/ and refresh latest pointers.

Scratch JSON lives in benchmark_results/ (gitignored). This script copies the
full hyperfine exports plus host/git metadata into a timestamped record that
is meant to be committed.

Does not overwrite benchmarks/summary.json unless --update-gate is passed.
That file is the CI regression baseline and should stay CI-measured.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path


SCENARIOS = ("search", "info", "status", "explicit", "update")


def repo_root() -> Path:
    here = Path(__file__).resolve().parent.parent
    if (here / "Cargo.toml").is_file():
        return here
    return Path.cwd()


def git_capture(root: Path) -> dict:
    def run(args: list[str]) -> str:
        try:
            return subprocess.check_output(args, cwd=root, text=True).strip()
        except (subprocess.CalledProcessError, FileNotFoundError):
            return ""

    status = run(["git", "status", "--porcelain"])
    return {
        "commit": run(["git", "rev-parse", "HEAD"]),
        "commit_short": run(["git", "rev-parse", "--short", "HEAD"]),
        "describe": run(["git", "describe", "--always", "--dirty"]),
        "branch": run(["git", "rev-parse", "--abbrev-ref", "HEAD"]),
        "dirty": bool(status),
        "dirty_files": [line[3:] for line in status.splitlines() if line.strip()],
    }


def host_capture() -> dict:
    cpu = ""
    cpuinfo = Path("/proc/cpuinfo")
    if cpuinfo.is_file():
        for line in cpuinfo.read_text(errors="replace").splitlines():
            if line.lower().startswith("model name"):
                cpu = line.split(":", 1)[1].strip()
                break
    mem_kib = 0
    meminfo = Path("/proc/meminfo")
    if meminfo.is_file():
        for line in meminfo.read_text(errors="replace").splitlines():
            if line.startswith("MemTotal:"):
                parts = line.split()
                if len(parts) >= 2:
                    mem_kib = int(parts[1])
                break
    uname = os.uname()
    return {
        "sysname": uname.sysname,
        "release": uname.release,
        "machine": uname.machine,
        "cpu": cpu,
        "ram_gib": round(mem_kib / (1024 * 1024), 1) if mem_kib else None,
    }


def load_json(path: Path) -> dict | None:
    if not path.is_file():
        return None
    with path.open() as handle:
        return json.load(handle)


def find_result(results: list[dict], name: str) -> dict | None:
    for result in results:
        if result.get("command") == name:
            return result
    return None


def ms(result: dict, key: str = "mean") -> float:
    return float(result[key]) * 1000.0


def summarize_scenario(data: dict) -> list[dict]:
    rows = []
    for result in data.get("results", []):
        times = result.get("times") or []
        exit_codes = result.get("exit_codes") or []
        rows.append(
            {
                "command": result.get("command"),
                "mean_ms": round(ms(result, "mean"), 3),
                "stddev_ms": round(ms(result, "stddev"), 3),
                "median_ms": round(ms(result, "median"), 3),
                "min_ms": round(ms(result, "min"), 3),
                "max_ms": round(ms(result, "max"), 3),
                "user_ms": round(ms(result, "user"), 3),
                "system_ms": round(ms(result, "system"), 3),
                "runs": len(times),
                "nonzero_exits": sum(1 for code in exit_codes if code != 0),
            }
        )
    return rows


def validate_results(source: Path) -> list[str]:
    """Fail closed if the run did not measure real work."""
    errors: list[str] = []
    present = [name for name in SCENARIOS if (source / f"{name}.json").is_file()]
    if not present:
        errors.append(f"no hyperfine JSON in {source}")
        return errors

    search = load_json(source / "search.json")
    if search is None:
        errors.append(f"search.json missing in {source}")
        return errors

    daemon = find_result(search["results"], "OMG (Daemon)")
    if daemon is None:
        errors.append('search.json has no command named "OMG (Daemon)"')
        return errors
    if any(code != 0 for code in daemon.get("exit_codes") or [1]):
        errors.append("OMG (Daemon) search had a non-zero exit")
    daemon_mean = ms(daemon)
    if daemon_mean < 1.0:
        errors.append(
            f"OMG (Daemon) search mean {daemon_mean:.2f} ms is too fast "
            "(command likely did no work)"
        )
    if daemon_mean > 500.0:
        errors.append(f"OMG (Daemon) search mean {daemon_mean:.1f} ms is implausibly slow")

    pacman = find_result(search["results"], "pacman")
    if pacman is not None:
        pacman_mean = ms(pacman)
        if pacman_mean < 30.0:
            errors.append(
                f"pacman search mean {pacman_mean:.1f} ms is too fast "
                "(likely not searching the sync databases)"
            )
        if daemon_mean >= pacman_mean:
            errors.append(
                f"OMG search ({daemon_mean:.1f} ms) was not faster than pacman "
                f"({pacman_mean:.1f} ms)"
            )
        if ms(pacman, "user") < 0.02:
            errors.append(
                f"pacman search user-time {ms(pacman, 'user'):.1f} ms is too low "
                "(CPU work missing)"
            )

    for name in SCENARIOS:
        payload = load_json(source / f"{name}.json")
        if payload is None:
            continue
        for result in payload.get("results", []):
            command = result.get("command", name)
            codes = result.get("exit_codes") or []
            if not codes:
                errors.append(f"{name}/{command}: no exit codes recorded")
            elif any(code != 0 for code in codes):
                errors.append(f"{name}/{command}: non-zero exit in timed runs")
            if not result.get("times"):
                errors.append(f"{name}/{command}: no timed runs")
    return errors


def render_latest_md(meta: dict, source: Path) -> str:
    git = meta["git"]
    host = meta["host"]
    lines = [
        "# OMG benchmark — latest recorded run",
        "",
        f"- **Record:** [`{meta['id']}`](records/{meta['id']}/)",
        f"- **When:** {meta['timestamp']}",
        f"- **Commit:** `{git.get('commit', '')}` (`{git.get('describe', '')}`)",
        f"- **Dirty tree:** {'yes' if git.get('dirty') else 'no'}",
        f"- **Host:** {host.get('cpu') or host.get('machine')}, "
        f"Linux {host.get('release')}, {host.get('ram_gib')} GiB RAM",
        f"- **Hyperfine:** {meta.get('hyperfine', 'unknown')}",
        f"- **Flags:** `--shell=none --output=pipe`, warmup={meta.get('warmup')}, "
        f"min-runs={meta.get('min_runs')}, max-runs={meta.get('max_runs')}",
        "",
        "Scratch output is gitignored (`benchmark_results/`). Canonical copies of the",
        "full hyperfine JSON (every run, every exit code) live under `benchmarks/records/`.",
        "",
        "## Preflight",
        "",
        "Each timed command was executed once before hyperfine. Search and info had to",
        "print `firefox`. Explicit count had to be a positive integer. Status had to",
        "succeed. Runs with a non-zero exit are rejected.",
        "",
    ]
    preflight = meta.get("preflight") or {}
    if preflight:
        lines.append("| Check | Evidence |")
        lines.append("|---|---|")
        for key, value in preflight.items():
            if isinstance(value, dict) and "bytes" in value:
                shown = f"{value['bytes']} bytes, {value.get('lines', 0)} lines"
            else:
                shown = value
            lines.append(f"| `{key}` | {shown} |")
        lines.append("")

    lines.extend(["## Results", ""])
    for name in SCENARIOS:
        md_path = source / f"{name}.md"
        json_path = source / f"{name}.json"
        if not md_path.is_file() and not json_path.is_file():
            continue
        lines.append(f"### {name}")
        lines.append("")
        if md_path.is_file():
            lines.append(md_path.read_text().rstrip())
            lines.append("")
        payload = load_json(json_path)
        if payload:
            daemon = find_result(payload["results"], "OMG (Daemon)")
            pacman = find_result(payload["results"], "pacman")
            if daemon and pacman and ms(daemon) > 0:
                speedup = ms(pacman) / ms(daemon)
                lines.append(
                    f"Daemon mean **{ms(daemon):.1f} ms** vs pacman **{ms(pacman):.1f} ms** "
                    f"({speedup:.1f}×). Median {ms(daemon, 'median'):.1f} ms "
                    f"({len(daemon.get('times') or [])} runs)."
                )
                lines.append("")

    lines.extend(
        [
            "## Reproduce",
            "",
            "```bash",
            "./benchmark-hyperfine.sh",
            "```",
            "",
        ]
    )
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        default="benchmark_results",
        help="Directory containing hyperfine JSON/MD (default: benchmark_results)",
    )
    parser.add_argument(
        "--update-gate",
        action="store_true",
        help="Also write benchmarks/summary.json from this run (CI only)",
    )
    parser.add_argument("--id", default="", help="Record id (default: UTC timestamp + short sha)")
    parser.add_argument("--warmup", type=int, default=None)
    parser.add_argument("--min-runs", type=int, default=None)
    parser.add_argument("--max-runs", type=int, default=None)
    args = parser.parse_args()

    root = repo_root()
    source = Path(args.source)
    if not source.is_absolute():
        source = (root / source).resolve()
    if not source.is_dir():
        print(f"No hyperfine output directory at {source}", file=sys.stderr)
        return 1

    errors = validate_results(source)
    if errors:
        print("Benchmark record rejected — numbers are not credible:", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1

    git = git_capture(root)
    host = host_capture()
    timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    stamp = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
    record_id = args.id or f"{stamp}-{git.get('commit_short') or 'nogit'}"
    records_dir = root / "benchmarks" / "records" / record_id
    records_dir.mkdir(parents=True, exist_ok=True)

    copied = []
    for path in sorted(source.glob("*")):
        if path.suffix in {".json", ".md"} and path.is_file():
            shutil.copy2(path, records_dir / path.name)
            copied.append(path.name)

    preflight = load_json(source / "preflight.json") or {}
    try:
        hyperfine = subprocess.check_output(["hyperfine", "--version"], text=True).strip()
    except (subprocess.CalledProcessError, FileNotFoundError):
        hyperfine = "unknown"

    scenarios = {}
    for name in SCENARIOS:
        payload = load_json(source / f"{name}.json")
        if payload:
            scenarios[name] = summarize_scenario(payload)

    search = load_json(source / "search.json") or {}
    daemon = find_result(search.get("results") or [], "OMG (Daemon)")
    pacman = find_result(search.get("results") or [], "pacman")
    search_ms = round(ms(daemon), 1) if daemon else None
    speedup = None
    if daemon and pacman and ms(daemon) > 0:
        speedup = f"{ms(pacman) / ms(daemon):.1f}x"

    meta = {
        "id": record_id,
        "timestamp": timestamp,
        "git": git,
        "host": host,
        "hyperfine": hyperfine,
        "warmup": args.warmup,
        "min_runs": args.min_runs,
        "max_runs": args.max_runs,
        "source_files": copied,
        "preflight": preflight,
        "scenarios": scenarios,
        "headline": {
            "search_mean_ms": search_ms,
            "search_median_ms": round(ms(daemon, "median"), 1) if daemon else None,
            "pacman_search_mean_ms": round(ms(pacman), 1) if pacman else None,
            "speedup": speedup,
        },
    }
    (records_dir / "meta.json").write_text(json.dumps(meta, indent=2) + "\n")

    latest = root / "benchmarks" / "latest.md"
    latest.write_text(render_latest_md(meta, source))

    if search_ms is not None:
        badge = {
            "schemaVersion": 1,
            "label": "search",
            "message": (
                f"{search_ms}ms ({speedup} faster)" if speedup else f"{search_ms}ms"
            ),
            "color": "brightgreen",
        }
        (root / "benchmarks" / "badge.json").write_text(
            json.dumps(badge, indent=2) + "\n"
        )

    if args.update_gate:
        if search_ms is None or not speedup:
            print("Cannot update gate: missing daemon or pacman search result", file=sys.stderr)
            return 1
        summary = {
            "timestamp": timestamp,
            "search_ms": search_ms,
            "speedup": speedup,
            "commit": git.get("commit", ""),
            "record": record_id,
        }
        (root / "benchmarks" / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")

    index_path = root / "benchmarks" / "records" / "INDEX.md"
    index_lines = [
        "# Benchmark records",
        "",
        "Newest first. Each folder is a full hyperfine export plus `meta.json`.",
        "",
        "| Record | Commit | Search mean | vs pacman | Host |",
        "|---|---|---:|---|---|",
    ]
    rows = []
    for entry in sorted((root / "benchmarks" / "records").iterdir(), reverse=True):
        meta_path = entry / "meta.json"
        if not meta_path.is_file():
            continue
        item = json.loads(meta_path.read_text())
        headline = item.get("headline") or {}
        git_info = item.get("git") or {}
        host_info = item.get("host") or {}
        rows.append(
            f"| [`{entry.name}`]({entry.name}/) | `{git_info.get('commit_short', '')}` | "
            f"{headline.get('search_mean_ms', '')} ms | {headline.get('speedup', '')} | "
            f"{(host_info.get('cpu') or '')[:40]} |"
        )
    index_path.write_text("\n".join(index_lines + rows) + "\n")

    print(f"Recorded {record_id}")
    print(f"  search mean: {search_ms} ms  speedup: {speedup}")
    print(f"  files: {records_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
