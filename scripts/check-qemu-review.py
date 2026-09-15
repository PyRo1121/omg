#!/usr/bin/env python3
"""Emit a bounded maintenance request before reviewed QEMU image pins expire."""
import argparse
from datetime import date, datetime, timezone
import json
from pathlib import Path


def review(manifest, today):
    if manifest.is_symlink() or manifest.stat().st_size > 65536:
        raise ValueError("invalid image manifest")
    data = json.loads(manifest.read_text())
    if data["schema_version"] != 1:
        raise ValueError("unsupported image manifest")
    start = date.fromisoformat(data["reviewed_on"])
    end = date.fromisoformat(data["review_expires"])
    if start > today or not 1 <= (end - start).days <= 31:
        raise ValueError("image review must cover at most 31 days and cannot start in the future")
    remaining = (end - today).days
    return {"schema_version": 1, "review_expires": end.isoformat(),
            "days_remaining": remaining, "review_required": remaining <= 7,
            "expired": remaining <= 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=Path("tests/qemu-image-provenance/manifest.json"))
    parser.add_argument("--github-output", type=Path)
    args = parser.parse_args()
    receipt = review(args.manifest, datetime.now(timezone.utc).date())
    print(json.dumps(receipt))
    if args.github_output:
        with args.github_output.open("a") as stream:
            stream.write(f"review_required={str(receipt['review_required']).lower()}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
