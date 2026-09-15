#!/usr/bin/env python3
"""Verify reviewed image provenance before any QEMU image parser runs."""
import argparse
from datetime import date, datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile
from urllib.parse import urlsplit


def material(root, name):
    if not re.fullmatch(r"[a-zA-Z0-9_.-]+", name):
        raise ValueError("invalid verification material name")
    path = root / name
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 1024 * 1024:
        raise ValueError("invalid verification material")
    return path.read_bytes()


def policy(manifest, identity, url, digest, today=None):
    data = json.loads(manifest.read_text())
    today = today or datetime.now(timezone.utc).date()
    reviewed = date.fromisoformat(data["reviewed_on"])
    expires = date.fromisoformat(data["review_expires"])
    if not 1 <= (expires - reviewed).days <= 31 or not reviewed <= today < expires:
        raise ValueError("image provenance review is expired or future-dated")
    entry = data["images"][identity]
    if entry["url"] != url or entry["digest"] != digest or not url.startswith("https://"):
        raise ValueError("image selection differs from reviewed provenance")
    if entry["algorithm"] not in ("sha256", "sha512"):
        raise ValueError("unsupported image digest algorithm")
    return entry


def verify_signature(entry, root, image, gpgv="gpgv"):
    kind = entry["signature_policy"]
    if kind == "publisher-unsigned-cloud-checksums":
        if entry["publisher"] != "debian" or not entry.get("rationale"):
            raise ValueError("unsigned publisher exception is not documented")
        return {"signature_verified": False, "signature_policy": kind}
    key = material(root, entry["keyring"])
    if hashlib.sha256(key).hexdigest() != entry["keyring_sha256"]:
        raise ValueError("public verification key changed")
    with tempfile.TemporaryDirectory() as directory:
        work = Path(directory)
        (work / "keyring.gpg").write_bytes(key)
        command = [gpgv, "--homedir", ".", "--keyring", "./keyring.gpg", "--status-fd", "1"]
        if kind == "image-detached":
            (work / "signature").write_bytes(material(root, entry["signature"]))
            command += ["signature", str(image.resolve())]
        elif kind == "checksum-detached":
            (work / "signature").write_bytes(material(root, entry["signature"]))
            (work / "checksums").write_bytes(material(root, entry["checksums"]))
            command += ["signature", "checksums"]
        elif kind == "checksum-clearsigned":
            (work / "signed-checksums").write_bytes(material(root, entry["checksums"]))
            command += ["--output", "checksums", "signed-checksums"]
        else:
            raise ValueError("unknown image signature policy")
        completed = subprocess.run(command, cwd=work, capture_output=True, text=True, timeout=60, check=False)
        valid = [line.split() for line in completed.stdout.splitlines() if line.startswith("[GNUPG:] VALIDSIG ")]
        if completed.returncode != 0 or not valid or any(row[-1] != entry["fingerprint"] for row in valid):
            raise ValueError("image publisher signature rejected")
        if kind != "image-detached":
            filename = Path(urlsplit(entry["url"]).path).name
            checksums = (work / "checksums").read_text()
            matches = []
            for line in checksums.splitlines():
                conventional = re.fullmatch(r"([0-9a-f]{64}) [ *](.+)", line)
                bsd = re.fullmatch(r"SHA256 \((.+)\) = ([0-9a-f]{64})", line)
                if conventional and conventional[2] == filename:
                    matches.append(conventional[1])
                elif bsd and bsd[1] == filename:
                    matches.append(bsd[2])
            if matches != [entry["digest"]]:
                raise ValueError("signed checksum does not uniquely match image pin")
    return {"signature_verified": True, "signature_policy": kind, "fingerprint": entry["fingerprint"]}


def verify(manifest, identity, url, digest, image, gpgv="gpgv"):
    entry = policy(manifest, identity, url, digest)
    if image.is_symlink() or not image.is_file():
        raise ValueError("image is not a regular file")
    with image.open("rb") as stream:
        actual = hashlib.file_digest(stream, entry["algorithm"]).hexdigest()
    if actual != entry["digest"]:
        raise ValueError("image digest rejected")
    signature = verify_signature(entry, manifest.parent, image, gpgv)
    return dict(schema_version=1, image=identity, digest=actual, image_verified=True, **signature)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--identity", required=True)
    parser.add_argument("--url", required=True)
    parser.add_argument("--digest", required=True)
    parser.add_argument("--image", required=True, type=Path)
    args = parser.parse_args()
    try:
        print(json.dumps(verify(args.manifest, args.identity, args.url, args.digest, args.image)))
        return 0
    except (ValueError, OSError, KeyError, subprocess.SubprocessError) as error:
        print(json.dumps({"schema_version": 1, "image_verified": False, "error": str(error)}))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
