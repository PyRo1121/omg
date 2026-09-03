#!/usr/bin/env python3
"""Check OMG's search benchmark for a credible performance regression."""

from __future__ import annotations

import json
import math
import os
import sys


def extract_command_time_from_hyperfine(json_path: str, command: str) -> float | None:
    """Extract one command's positive mean time in milliseconds."""
    try:
        with open(json_path, encoding="utf-8") as handle:
            data = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        print(f"Note: Could not parse hyperfine JSON: {error}")
        return None

    if not isinstance(data, dict) or not isinstance(data.get("results"), list):
        return None
    for result in data["results"]:
        if not isinstance(result, dict) or result.get("command") != command:
            continue
        mean = result.get("mean")
        if isinstance(mean, bool) or not isinstance(mean, (int, float)):
            return None
        milliseconds = float(mean) * 1000
        return milliseconds if math.isfinite(milliseconds) and milliseconds > 0 else None
    return None


def extract_search_time_from_hyperfine(json_path: str) -> float | None:
    """Extract the daemon search mean from hyperfine JSON."""
    return extract_command_time_from_hyperfine(json_path, "OMG (Daemon)")


def parse_positive_number(value: object) -> float | None:
    """Parse a positive number, optionally suffixed with ``x``."""
    if isinstance(value, bool) or not isinstance(value, (str, int, float)):
        return None
    if isinstance(value, str):
        value = value.removesuffix("x").strip()
    try:
        parsed = float(value)
    except ValueError:
        return None
    return parsed if math.isfinite(parsed) and parsed > 0 else None


def extract_search_time_from_markdown(markdown_path: str) -> float | None:
    """Extract the daemon search time from the legacy Markdown report."""
    try:
        with open(markdown_path, encoding="utf-8") as handle:
            for line in handle:
                if "| search |" not in line:
                    continue
                parts = line.split("|")
                milliseconds = float(parts[2].strip().removesuffix("ms"))
                return milliseconds if math.isfinite(milliseconds) and milliseconds > 0 else None
    except (OSError, IndexError, ValueError) as error:
        print(f"Note: Could not parse benchmark Markdown: {error}")
    return None


def check_regression() -> int:
    """Return nonzero only for invalid evidence or a credible regression."""
    baseline_path = "benchmarks/summary.json"
    hyperfine_json_paths = (
        "benchmark_results/search.json",
        "benchmark_results.json",
    )
    markdown_report_path = "benchmark_report.md"

    require_baseline = os.environ.get("OMG_PERF_REQUIRE_BASELINE", "").strip() in {
        "1",
        "true",
        "yes",
    }
    if not os.path.exists(baseline_path):
        if require_baseline:
            print(f"Failing closed: required baseline missing at {baseline_path}")
            return 1
        print("No baseline found. Skipping regression check.")
        return 0

    try:
        with open(baseline_path, encoding="utf-8") as handle:
            baseline = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        print(f"Failing closed: could not load baseline {baseline_path}: {error}")
        return 1
    if not isinstance(baseline, dict):
        print(f"Failing closed: baseline {baseline_path} must be a JSON object")
        return 1

    current_search_ms = None
    current_pacman_ms = None
    for json_path in hyperfine_json_paths:
        current_search_ms = extract_search_time_from_hyperfine(json_path)
        if current_search_ms is not None:
            current_pacman_ms = extract_command_time_from_hyperfine(json_path, "pacman")
            break

    if current_search_ms is None:
        current_search_ms = extract_search_time_from_markdown(markdown_report_path)
    if current_search_ms is None:
        print("❌ Could not extract current search time from any source.")
        return 1

    baseline_search_ms = parse_positive_number(baseline.get("search_ms"))
    if baseline_search_ms is None:
        print("Invalid baseline search time.")
        return 1

    try:
        threshold = float(os.environ.get("OMG_PERF_THRESHOLD", "1.35"))
    except ValueError:
        print("Invalid OMG_PERF_THRESHOLD; using default 1.35")
        threshold = 1.35
    if not math.isfinite(threshold) or threshold <= 1.0:
        print("OMG_PERF_THRESHOLD must be finite and > 1.0; using default 1.35")
        threshold = 1.35

    print(f"Baseline Search: {baseline_search_ms}ms")
    print(f"Current Search: {current_search_ms}ms")

    absolute_regression = current_search_ms > baseline_search_ms * threshold
    baseline_speedup = parse_positive_number(baseline.get("speedup"))
    current_speedup = None
    if current_pacman_ms is not None:
        current_speedup = current_pacman_ms / current_search_ms
    if baseline_speedup is not None and current_speedup is not None:
        print(f"Baseline speedup vs pacman: {baseline_speedup:.2f}x")
        print(f"Current speedup vs pacman: {current_speedup:.2f}x")

    relative_regression = (
        baseline_speedup is not None
        and current_speedup is not None
        and current_speedup < baseline_speedup / threshold
    )
    if absolute_regression and (
        baseline_speedup is None or current_speedup is None or relative_regression
    ):
        difference = ((current_search_ms / baseline_search_ms) - 1) * 100
        print("❌ PERFORMANCE REGRESSION DETECTED!")
        print(f"Search time increased by {difference:.2f}% (exceeds configured threshold)")
        return 1

    if absolute_regression:
        print("Absolute time moved with the in-run pacman control; treating it as runner noise.")
    print("✅ Performance check passed.")
    return 0


if __name__ == "__main__":
    sys.exit(check_regression())
