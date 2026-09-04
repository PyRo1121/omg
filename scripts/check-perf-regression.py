#!/usr/bin/env python3
"""Check OMG's search benchmark for a credible performance regression."""

from __future__ import annotations

from dataclasses import dataclass
import json
import math
import os
import sys


@dataclass(frozen=True, slots=True)
class Measurement:
    mean_ms: float
    stddev_ms: float | None
    samples: int | None


def extract_command_measurement(json_path: str, command: str) -> Measurement | None:
    """Extract one command's timing distribution from Hyperfine JSON."""
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
        mean_ms = float(mean) * 1000
        if not math.isfinite(mean_ms) or mean_ms <= 0:
            return None

        stddev = result.get("stddev")
        stddev_ms = (
            float(stddev) * 1000
            if not isinstance(stddev, bool)
            and isinstance(stddev, (int, float))
            and math.isfinite(float(stddev))
            and stddev >= 0
            else None
        )
        times = result.get("times")
        samples = len(times) if isinstance(times, list) and len(times) > 1 else None
        return Measurement(mean_ms=mean_ms, stddev_ms=stddev_ms, samples=samples)
    return None


def confidence_bounds(measurement: Measurement) -> tuple[float, float]:
    """Return an approximate 95% confidence interval for the measured mean."""
    if measurement.stddev_ms is None or measurement.samples is None:
        return measurement.mean_ms, measurement.mean_ms
    margin = 1.96 * measurement.stddev_ms / math.sqrt(measurement.samples)
    return max(measurement.mean_ms - margin, sys.float_info.min), measurement.mean_ms + margin


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

    current_search = None
    current_pacman = None
    for json_path in hyperfine_json_paths:
        current_search = extract_command_measurement(json_path, "OMG (Daemon)")
        if current_search is not None:
            current_pacman = extract_command_measurement(json_path, "pacman")
            break

    if current_search is None:
        markdown_search_ms = extract_search_time_from_markdown(markdown_report_path)
        if markdown_search_ms is not None:
            current_search = Measurement(markdown_search_ms, None, None)
    if current_search is None:
        print("❌ Could not extract current search time from any source.")
        return 1
    status_path = "benchmark_results/status.json"
    current_status = (
        extract_command_measurement(status_path, "OMG (Daemon)")
        if os.path.exists(status_path)
        else None
    )

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
    print(f"Current Search: {current_search.mean_ms}ms")

    search_lower, search_upper = confidence_bounds(current_search)
    if search_lower != search_upper:
        print(f"Current search mean 95% interval: {search_lower:.2f}ms to {search_upper:.2f}ms")

    absolute_limit = baseline_search_ms * threshold
    absolute_regression = current_search.mean_ms > absolute_limit
    credible_absolute_regression = search_lower > absolute_limit
    baseline_speedup = parse_positive_number(baseline.get("speedup"))
    current_speedup = None
    maximum_current_speedup = None
    if current_pacman is not None:
        current_speedup = current_pacman.mean_ms / current_search.mean_ms
        _, pacman_upper = confidence_bounds(current_pacman)
        maximum_current_speedup = pacman_upper / search_lower
    if baseline_speedup is not None and current_speedup is not None:
        print(f"Baseline speedup vs pacman: {baseline_speedup:.2f}x")
        print(f"Current speedup vs pacman: {current_speedup:.2f}x")

    relative_limit = baseline_speedup / threshold if baseline_speedup is not None else None
    relative_regression = (
        relative_limit is not None
        and current_speedup is not None
        and current_speedup < relative_limit
    )
    credible_relative_regression = (
        relative_limit is not None
        and maximum_current_speedup is not None
        and maximum_current_speedup < relative_limit
    )

    baseline_status_ms = parse_positive_number(baseline.get("status_ms"))
    matched_control_available = False
    matched_regression = False
    credible_matched_regression = False
    if baseline_status_ms is not None and current_status is not None:
        matched_control_available = True
        baseline_search_to_status = baseline_search_ms / baseline_status_ms
        current_search_to_status = current_search.mean_ms / current_status.mean_ms
        _, status_upper = confidence_bounds(current_status)
        minimum_search_to_status = search_lower / status_upper
        matched_limit = baseline_search_to_status * threshold
        matched_regression = current_search_to_status > matched_limit
        credible_matched_regression = minimum_search_to_status > matched_limit
        print(f"Baseline search/status ratio: {baseline_search_to_status:.2f}x")
        print(f"Current search/status ratio: {current_search_to_status:.2f}x")

    pacman_confirms = (
        baseline_speedup is None
        or current_speedup is None
        or credible_relative_regression
    )
    status_confirms = (
        baseline_status_ms is None
        or current_status is None
        or credible_matched_regression
    )
    if credible_absolute_regression and pacman_confirms and status_confirms:
        difference = ((current_search.mean_ms / baseline_search_ms) - 1) * 100
        print("❌ PERFORMANCE REGRESSION DETECTED!")
        print(f"Search time increased by {difference:.2f}% (exceeds configured threshold)")
        return 1

    if (
        absolute_regression
        and relative_regression
        and matched_control_available
        and not matched_regression
    ):
        print("Search stayed within the matched status control; treating the broad slowdown as runner noise.")
    elif absolute_regression and relative_regression:
        print("Point estimates cross all limits, but measurement uncertainty overlaps a control limit.")
    elif absolute_regression:
        print("Absolute time moved with the in-run pacman control; treating it as runner noise.")
    print("✅ Performance check passed.")
    return 0


if __name__ == "__main__":
    sys.exit(check_regression())
