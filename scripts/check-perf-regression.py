#!/usr/bin/env python3
import json
import sys
import os

def check_regression():
    latest_path = 'benchmarks/summary.json'
    current_report_path = 'benchmark_report.md'
    
    if not os.path.exists(latest_path):
        print("No baseline found. Skipping regression check.")
        return 0

    try:
        with open(latest_path, 'r') as f:
            baseline = json.load(f)
    except Exception as e:
        print(f"Error loading baseline: {e}")
        return 0

    # Extract current search time from report
    current_search_ms = None
    try:
        with open(current_report_path, 'r') as f:
            for line in f:
                if '| search |' in line:
                    parts = line.split('|')
                    # Format: | Command | OMG (Daemon) | pacman | yay | Speedup vs pacman |
                    # index 2 is OMG (Daemon)
                    val = parts[2].strip().replace('ms', '')
                    current_search_ms = float(val)
                    break
    except Exception as e:
        print(f"Error parsing current report: {e}")
        return 1

    if current_search_ms is None:
        print("Could not find search performance in current report.")
        return 1

    baseline_search_ms = baseline.get('search_ms')
    if baseline_search_ms is None or baseline_search_ms == 0:
        print("Invalid baseline search time.")
        return 0

    threshold = 1.15 # 15% tolerance
    print(f"Baseline Search: {baseline_search_ms}ms")
    print(f"Current Search: {current_search_ms}ms")
    
    if current_search_ms > baseline_search_ms * threshold:
        diff = ((current_search_ms / baseline_search_ms) - 1) * 100
        print(f"❌ PERFORMANCE REGRESSION DETECTED!")
        print(f"Search time increased by {diff:.2f}% (exceeds 15% threshold)")
        return 1
    
    print("✅ Performance check passed.")
    return 0

if __name__ == "__main__":
    sys.exit(check_regression())
