#!/usr/bin/env python3
"""
Performance regression checker for OMG benchmarks.

Supports both hyperfine JSON output and markdown report parsing.
Checks for performance regressions against historical baseline.
"""
import json
import math
import os
import sys

def extract_search_time_from_hyperfine(json_path):
    """Extract search time from hyperfine JSON output."""
    try:
        with open(json_path, 'r') as f:
            data = json.load(f)
        
        # Find the OMG search result (benchmark-hyperfine.sh exports it with
        # --command-name "OMG (Daemon)")
        for result in data.get('results', []):
            if 'omg' in result.get('command', '').lower():
                # Convert seconds to milliseconds
                return result['mean'] * 1000
        
        return None
    except Exception as e:
        print(f"Note: Could not parse hyperfine JSON: {e}")
        return None

def extract_search_time_from_markdown(md_path):
    """Extract search time from markdown benchmark report."""
    try:
        with open(md_path, 'r') as f:
            for line in f:
                if '| search |' in line:
                    parts = line.split('|')
                    # Format: | Command | OMG (Daemon) | pacman | yay | Speedup vs pacman |
                    # index 2 is OMG (Daemon)
                    val = parts[2].strip().replace('ms', '')
                    return float(val)
        return None
    except Exception as e:
        print(f"Error parsing markdown report: {e}")
        return None

def check_regression():
    latest_path = 'benchmarks/summary.json'
    hyperfine_json_paths = [
        'benchmark_results/search.json',
        'benchmark_results.json',
    ]
    markdown_report_path = 'benchmark_report.md'
    
    if not os.path.exists(latest_path):
        print("No baseline found. Skipping regression check.")
        return 0

    try:
        with open(latest_path, 'r') as f:
            baseline = json.load(f)
    except Exception as e:
        print(f"Failing closed: could not load baseline {latest_path}: {e}")
        return 1

    current_search_ms = None
    for json_path in hyperfine_json_paths:
        current_search_ms = extract_search_time_from_hyperfine(json_path)
        if current_search_ms is not None:
            break
    
    if current_search_ms is None:
        current_search_ms = extract_search_time_from_markdown(markdown_report_path)
    
    if current_search_ms is None:
        print("❌ Could not extract current search time from any source.")
        return 1

    try:
        baseline_search_ms = float(baseline.get('search_ms', 0))
    except (TypeError, ValueError):
        baseline_search_ms = 0.0
    if not math.isfinite(baseline_search_ms) or baseline_search_ms <= 0:
        print("Invalid baseline search time.")
        return 1

    # Default tolerance 35%: tight enough to catch real regressions, loose
    # enough for shared-runner noise. Override via OMG_PERF_THRESHOLD.
    try:
        threshold = float(os.environ.get("OMG_PERF_THRESHOLD", "1.35"))
    except ValueError:
        print("Invalid OMG_PERF_THRESHOLD; using default 1.35")
        threshold = 1.35
    if threshold <= 1.0:
        print("OMG_PERF_THRESHOLD must be > 1.0; using default 1.35")
        threshold = 1.35
    print(f"Baseline Search: {baseline_search_ms}ms")
    print(f"Current Search: {current_search_ms}ms")

    if current_search_ms > baseline_search_ms * threshold:
        diff = ((current_search_ms / baseline_search_ms) - 1) * 100
        print(f"❌ PERFORMANCE REGRESSION DETECTED!")
        print(f"Search time increased by {diff:.2f}% (exceeds configured threshold)")
        return 1
    
    print("✅ Performance check passed.")
    return 0

if __name__ == "__main__":
    sys.exit(check_regression())
