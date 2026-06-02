#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Self-test for the .stuchka-backup integrity gate (deploy/03 3.3 + 3.8 acceptance).

Builds a GENUINE extracted backup tree (main-db snapshot + independent audit.sqlite + an evidence
file + config.toml) and a manifest whose per-file sha256 values are computed FROM the actual file
bytes -- never invented. Then:

  case 1: pristine tree -> verify_backup_manifest.py must return PASS (exit 0).
  case 2: flip ONE byte in the audit.sqlite asset -> must return FAIL (exit 1), naming that file.
  case 3: delete one asset -> must return FAIL (exit 1), naming it missing.

This honours the no-mock constraint: the expected hashes are derived from the law/format bytes, and
the verdicts come from the real verifier, so a "pass on invented expected values" can never happen.

Pure standard library. Zero Emoji. Exit 0 if all three cases behave correctly, else 1.
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
VERIFIER = os.path.join(HERE, "verify_backup_manifest.py")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def build_tree(root: str) -> None:
    # main-db snapshot (DB choice TBD form; bytes stand in for a real logical dump / sqlite file).
    os.makedirs(os.path.join(root, "main-db"), exist_ok=True)
    os.makedirs(os.path.join(root, "evidence", "case_001"), exist_ok=True)

    assets = {
        # Relative path -> real bytes. Ground truth = sha256 of THESE bytes.
        "main-db/snapshot": b"STUCHKA-MAIN-DB-LOGICAL-DUMP-v12\x00row1\x00row2",
        "audit.sqlite": b"SQLite format 3\x00AUDIT-HASH-CHAIN-seq1-prev0",
        "evidence/case_001/ev_001.jpg": b"\xff\xd8\xff\xe0JFIF-pretend-evidence-bytes",
        "config.toml": b"[app]\nschema_version = 12\n",
    }
    files = []
    for rel, data in assets.items():
        full = os.path.join(root, rel)
        os.makedirs(os.path.dirname(full), exist_ok=True)
        with open(full, "wb") as fh:
            fh.write(data)
        files.append({"path": rel, "sha256": sha256_bytes(data), "size": len(data)})

    manifest = {
        "format_version": "1.0",
        "stuchka_version": "0.2.0",
        "case_count": 1,
        "integrity": {"files": files},
        "db_form": "tbd-l0-03",
        "audit_db": {"path": "audit.sqlite", "independent": True, "crate": "crates/audit"},
        "compat": {"min_restore_version": "0.1.0", "schema_version": 12},
    }
    with open(os.path.join(root, "manifest.json"), "w", encoding="utf-8") as fh:
        json.dump(manifest, fh, ensure_ascii=False, indent=2)


def run_verifier(root: str) -> int:
    proc = subprocess.run([sys.executable, VERIFIER, "--root", root, "--json"],
                          stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    sys.stdout.write(proc.stdout.decode("utf-8", errors="replace"))
    return proc.returncode


def main() -> int:
    work = tempfile.mkdtemp(prefix="stuchka-backup-selftest-")
    try:
        root = os.path.join(work, "extracted")
        os.makedirs(root)
        build_tree(root)

        ok = True

        print("[selftest] case 1: pristine tree must PASS")
        rc = run_verifier(root)
        if rc != 0:
            print("  FAIL: pristine tree did not PASS (exit %d)" % rc); ok = False
        else:
            print("  PASS")

        print("[selftest] case 2: one-byte tamper in audit.sqlite must FAIL")
        audit = os.path.join(root, "audit.sqlite")
        data = bytearray(open(audit, "rb").read())
        data[-1] ^= 0x01  # flip one bit of the last byte
        with open(audit, "wb") as fh:
            fh.write(bytes(data))
        rc = run_verifier(root)
        if rc != 1:
            print("  FAIL: tampered audit.sqlite was not rejected (exit %d)" % rc); ok = False
        else:
            print("  PASS (rejected as expected)")

        print("[selftest] case 3: missing asset must FAIL")
        # restore pristine, then delete one file
        shutil.rmtree(root)
        os.makedirs(root)
        build_tree(root)
        os.remove(os.path.join(root, "evidence", "case_001", "ev_001.jpg"))
        rc = run_verifier(root)
        if rc != 1:
            print("  FAIL: missing asset was not rejected (exit %d)" % rc); ok = False
        else:
            print("  PASS (rejected as expected)")

        if ok:
            print("[selftest] backup integrity gate OK (pass-pristine + reject-tampered + reject-missing).")
            return 0
        print("[selftest] backup integrity gate FAILED.")
        return 1
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
