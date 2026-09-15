#!/usr/bin/env python3
"""Exercise privacy-export atomic writes inside an explicitly marked disposable VM."""
import argparse
import errno
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import tempfile


def command(argv, **kwargs):
    return subprocess.run(argv, timeout=30, check=True, capture_output=True, **kwargs)


def validate_receipt(data):
    expected = {"enospc", "readonly", "fsync-eio", "fsync-kill"}
    if not isinstance(data, dict):
        raise ValueError("invalid storage fault receipt")
    rows = data.get("cases", [])
    if (data.get("schema_version") != 1 or data.get("scope") != "privacy-export-atomic-write"
            or data.get("complete") is not True or not isinstance(rows, list)
            or len(rows) != len(expected) or not all(isinstance(row, dict) for row in rows)
            or {row.get("id") for row in rows} != expected
            or any(row.get("fault_observed") is not True or row.get("prior_preserved") is not True
                   or row.get("recovered") is not True for row in rows)):
        raise ValueError("incomplete storage fault coverage")


def run(binary, token):
    import pwd
    marker = Path("/run/omg-qemu-storage-faults")
    if (os.geteuid() != 0 or marker.is_symlink() or not marker.is_file()
            or marker.stat().st_uid != 0 or stat.S_IMODE(marker.stat().st_mode) != 0o444
            or not re.fullmatch(r"[0-9a-f-]{36}", token) or marker.read_text().strip() != token):
        raise ValueError("explicit disposable-guest marker required")
    if command(["systemd-detect-virt", "--vm"], text=True).stdout.strip() not in ("kvm", "qemu"):
        raise ValueError("QEMU guest required")
    account = pwd.getpwnam("bench")
    binary = binary.resolve(strict=True)
    if not binary.is_relative_to(Path(account.pw_dir)) or binary.name != "omg":
        raise ValueError("guest release binary required")
    root = Path(tempfile.mkdtemp(prefix="omg-storage-faults-", dir=account.pw_dir))
    os.chown(root, account.pw_uid, account.pw_gid)
    target = root / "volume"
    target.mkdir()
    mounted = False
    rows = []
    try:
        command(["mount", "-t", "tmpfs", "-o",
                 f"size=1m,mode=0700,uid={account.pw_uid},gid={account.pw_gid},nodev,nosuid,noexec",
                 "omg-storage-faults", str(target)])
        mounted = True
        env = {"PATH": "/usr/bin:/bin", "HOME": str(root), "USER": "bench", "LOGNAME": "bench",
               "XDG_CONFIG_HOME": str(root / "config"), "XDG_DATA_HOME": str(root / "data"),
               "XDG_CACHE_HOME": str(root / "cache"), "OMG_DATA_DIR": str(root / "data"),
               "CI": "1", "OMG_NO_UPDATE_CHECK": "1", "OMG_TELEMETRY": "0", "NO_COLOR": "1"}
        output = target / "export.json"
        prefix = ["setpriv", f"--reuid={account.pw_uid}", f"--regid={account.pw_gid}",
                  "--clear-groups", "--no-new-privs"]
        product = [str(binary), "privacy", "export", "--output", str(output)]

        def export(extra=()):
            return subprocess.run(prefix + list(extra) + product, cwd=root, env=env,
                                  capture_output=True, timeout=30, check=False)

        def recovered():
            if export().returncode != 0:
                raise ValueError("export did not recover after fault removal")
            json.loads(output.read_text())
            if stat.S_IMODE(output.stat().st_mode) != 0o600:
                raise ValueError("recovered export is not private")

        recovered()
        for fault in ("enospc", "readonly", "fsync-eio", "fsync-kill"):
            before = output.read_bytes()
            trace = root / "sync.trace"
            observed = False
            if fault == "enospc":
                try:
                    with (target / "filler").open("wb", buffering=0) as stream:
                        for _ in range(512):
                            stream.write(b"x" * 4096)
                except OSError as error:
                    if error.errno != errno.ENOSPC:
                        raise
                    observed = True
                result = export()
                (target / "filler").unlink()
            elif fault == "readonly":
                command(["mount", "-o", "remount,ro", str(target)])
                try:
                    (target / "probe").write_bytes(b"x")
                except OSError as error:
                    if error.errno != errno.EROFS:
                        raise
                    observed = True
                result = export()
                command(["mount", "-o", "remount,rw", str(target)])
            else:
                injection = "error=EIO" if fault == "fsync-eio" else "signal=SIGKILL"
                result = export(["strace", "-f", "-yy", "-o", str(trace),
                                 "-e", "trace=fsync,fdatasync",
                                 "-e", f"inject=fsync:{injection}:when=1"])
                text = trace.read_text()
                # Require the interrupted sync to concern a file in this test
                # volume, not unrelated initialization before the export.
                observed = any("fsync(" in line and str(target) + "/" in line
                               and ("EIO" in line if fault == "fsync-eio" else True)
                               for line in text.splitlines())
                if fault == "fsync-eio":
                    observed = observed and "(INJECTED)" in text
                else:
                    observed = observed and "killed by SIGKILL" in text and result.returncode in (-9, 137)
            if not observed or result.returncode == 0 or output.read_bytes() != before:
                raise ValueError(f"{fault}: activation, refusal or prior export integrity not proven")
            recovered()
            rows.append(dict(id=fault, fault_observed=True, prior_preserved=True, recovered=True))
        receipt = dict(schema_version=1, scope="privacy-export-atomic-write", complete=True, cases=rows)
        validate_receipt(receipt)
        return receipt
    finally:
        if mounted:
            command(["umount", str(target)])
        # Only this invocation's guest-owned temporary directory is removed.
        shutil.rmtree(root)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--token")
    parser.add_argument("--receipt", type=Path)
    args = parser.parse_args()
    if args.receipt:
        if args.receipt.is_symlink() or args.receipt.stat().st_size > 65536:
            raise ValueError("invalid storage evidence")
        validate_receipt(json.loads(args.receipt.read_text()))
        return
    if not args.binary or not args.token:
        parser.error("--binary and --token are required inside the guest")
    print(json.dumps(run(args.binary, args.token)))


if __name__ == "__main__":
    main()
