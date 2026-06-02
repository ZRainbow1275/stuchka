#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Standalone .stuchka-backup manifest integrity verifier (deploy/03 3.3 + 3.8).

Spec: deploy/03-disaster-recovery.md 3.2.2 (manifest.json shape) + 3.3.1 (per-file sha256 bound
to the manifest) + 3.8 acceptance: "break one byte -> FAIL". This is the REAL gate, not a mock:
it walks the EXTRACTED backup tree (after age-decrypt + zstd + tar) and recomputes every file's
SHA-256, comparing against `manifest.integrity.files[].sha256`. One tampered byte flips the verdict.

It does NOT decrypt (age is the documented seam) -- it verifies an already-extracted staging tree,
which is what stuchka-core's --verify-backup does internally and what the restore script extracts.
A companion self-test (scripts/recovery/selftest_backup.py) builds a genuine tree + manifest with
known-good hashes derived FROM the file bytes (never invented), proves PASS, then flips one byte and
proves FAIL.

Pure standard library. Zero Emoji. Cross-machine.

Usage:
  python verify_backup_manifest.py --root <extracted-dir>           # human-readable report
  python verify_backup_manifest.py --root <extracted-dir> --json    # machine-readable, same exit code

Exit codes: 0 PASS (all files match) ; 1 FAIL (>=1 corrupted/missing) ; 2 manifest missing/invalid.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys


def sha256_file(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def verify(root: str) -> dict:
    manifest_path = os.path.join(root, "manifest.json")
    if not os.path.isfile(manifest_path):
        return {"status": "ERROR", "error": "manifest.json not found in %s" % root,
                "files_verified": 0, "corrupted": [], "missing": []}
    try:
        with open(manifest_path, "r", encoding="utf-8") as fh:
            manifest = json.load(fh)
    except (ValueError, OSError) as exc:
        return {"status": "ERROR", "error": "manifest.json unreadable: %s" % exc,
                "files_verified": 0, "corrupted": [], "missing": []}

    integrity = manifest.get("integrity", {})
    files = integrity.get("files", [])
    if not isinstance(files, list) or not files:
        return {"status": "ERROR", "error": "manifest.integrity.files is empty/invalid",
                "files_verified": 0, "corrupted": [], "missing": []}

    corrupted = []
    missing = []
    verified = 0
    for entry in files:
        rel = entry.get("path")
        expected = (entry.get("sha256") or "").lower()
        if not rel or not expected:
            corrupted.append(str(rel))
            continue
        full = os.path.join(root, rel)
        if not os.path.isfile(full):
            missing.append(rel)
            continue
        actual = sha256_file(full)
        if actual != expected:
            corrupted.append(rel)
        else:
            verified += 1

    status = "PASS" if not corrupted and not missing else "FAIL"
    return {
        "status": status,
        "stuchka_version": manifest.get("stuchka_version"),
        "case_count": manifest.get("case_count"),
        "files_total": len(files),
        "files_verified": verified,
        "corrupted": corrupted,
        "missing": missing,
        "schema_version": manifest.get("compat", {}).get("schema_version"),
    }


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description="Verify an extracted .stuchka-backup tree against its manifest.")
    parser.add_argument("--root", required=True, help="Extracted backup directory (contains manifest.json).")
    parser.add_argument("--json", action="store_true", help="Machine-readable output.")
    args = parser.parse_args(argv)

    report = verify(args.root)

    if args.json:
        sys.stdout.write(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    else:
        sys.stdout.write("=== Stucka Backup Verification (manifest-bound sha256) ===\n")
        sys.stdout.write("Root: %s\n" % args.root)
        if report["status"] == "ERROR":
            sys.stdout.write("Status: ERROR - %s\n" % report["error"])
        else:
            sys.stdout.write("Stuchka Version: %s\n" % report.get("stuchka_version"))
            sys.stdout.write("Files Verified: %d / %d\n" % (report["files_verified"], report["files_total"]))
            sys.stdout.write("Files Corrupted: %d\n" % len(report["corrupted"]))
            sys.stdout.write("Files Missing: %d\n" % len(report["missing"]))
            for c in report["corrupted"]:
                sys.stdout.write("  CORRUPTED: %s\n" % c)
            for m in report["missing"]:
                sys.stdout.write("  MISSING: %s\n" % m)
            sys.stdout.write("Status: %s\n" % report["status"])

    if report["status"] == "PASS":
        return 0
    if report["status"] == "ERROR":
        return 2
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
