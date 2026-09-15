#!/usr/bin/env python3
"""Collect bounded guest health and reject crash evidence before QEMU admission.

Only kernel messages and selected coredump identity fields are queried. Never
collect core files, process environments, command lines or arbitrary journals.
"""
import argparse
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import tempfile

LIMIT = 1024 * 1024
FATAL = re.compile(
    r"(?:Kernel panic - not syncing:|BUG: (?:unable to handle|kernel NULL|soft lockup)|"
    r"Oops:|watchdog: BUG: soft lockup|NMI watchdog: Watchdog detected hard LOCKUP|"
    r"Out of memory: Killed process|Memory cgroup out of memory: Killed process|"
    r"(?:omg|omgd)\[[0-9]+\]: (?:segfault|general protection fault))"
)
BOOT_ID = re.compile(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}")


def bounded_file(path):
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0))
    with os.fdopen(descriptor, "rb") as stream:
        info = os.fstat(stream.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_size > LIMIT:
            raise ValueError("invalid or oversized health evidence")
        data = stream.read(LIMIT + 1)
    if len(data) > LIMIT:
        raise ValueError("health evidence grew beyond limit")
    return data.decode("utf-8", errors="strict")


def query(argv):
    # File-backed capture bounds Python memory even if a guest is very noisy.
    with tempfile.TemporaryFile() as output:
        completed = subprocess.run(argv, stdout=output, stderr=subprocess.DEVNULL,
                                   timeout=15, check=False)
        if completed.returncode != 0 or output.tell() > LIMIT:
            raise ValueError("health query failed or exceeded limit")
        output.seek(0)
        return output.read(LIMIT + 1).decode("utf-8", errors="strict")


def crash_signatures(text):
    # Publish only the matched signature, never adjacent potentially sensitive text.
    return sorted(set(match.group(0) for match in FATAL.finditer(text)))


def collect():
    boot_id = Path("/proc/sys/kernel/random/boot_id").read_text().strip()
    if not BOOT_ID.fullmatch(boot_id):
        raise ValueError("invalid guest boot identity")
    kernel = query(["journalctl", "--boot", boot_id, "--dmesg", "--no-pager", "--output=cat"])
    if not kernel.strip() or kernel.strip() == "-- No entries --":
        raise ValueError("missing boot kernel evidence")
    cores = query(["journalctl", "--boot", boot_id, "--no-pager", "--output=json",
                   "--output-fields=COREDUMP_COMM,COREDUMP_SIGNAL",
                   "MESSAGE_ID=fc2e22bc6ee647b6b90729ab34a250b1"])
    crashes = []
    for line in cores.splitlines():
        row = json.loads(line)
        if row.get("COREDUMP_COMM") in ("omg", "omgd"):
            signal = row.get("COREDUMP_SIGNAL")
            if not isinstance(signal, str) or not re.fullmatch(r"[0-9]{1,3}", signal):
                raise ValueError("invalid crash signal")
            crashes.append({"process": row["COREDUMP_COMM"], "signal": int(signal)})
    return {"schema_version": 1, "complete": True, "boot_id": boot_id,
            "kernel_bytes": len(kernel.encode()), "fatal_signatures": crash_signatures(kernel),
            "product_crashes": crashes}


def verify(guest, serial, controller):
    payload = json.loads(bounded_file(guest))
    if (not isinstance(payload, dict) or payload.get("schema_version") != 1
            or payload.get("complete") is not True
            or not isinstance(payload.get("boot_id"), str)
            or not BOOT_ID.fullmatch(payload["boot_id"])
            or type(payload.get("kernel_bytes")) is not int
            or not 0 < payload["kernel_bytes"] <= LIMIT
            or payload.get("fatal_signatures") != []
            or payload.get("product_crashes") != []):
        raise ValueError("guest crash or incomplete health evidence")
    serial_text = bounded_file(serial)
    if not serial_text.strip() or crash_signatures(serial_text):
        raise ValueError("missing serial evidence or fatal guest signature")
    state = json.loads(bounded_file(controller))
    if (not isinstance(state, dict) or state.get("Running") is not True
            or state.get("OOMKilled") is not False
            or type(state.get("ExitCode")) is not int or state["ExitCode"] != 0):
        raise ValueError("controller stopped or was OOM killed")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("collect")
    admission = commands.add_parser("verify")
    for name in ("guest", "serial", "controller"):
        admission.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "collect":
            print(json.dumps(collect()))
            return 0
        return verify(args.guest, args.serial, args.controller)
    except (OSError, ValueError, subprocess.TimeoutExpired):
        print("QEMU health evidence failed admission", file=__import__("sys").stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
