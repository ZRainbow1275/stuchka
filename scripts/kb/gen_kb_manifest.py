#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Generate a KB distribution manifest + checksums (deploy/04 4.2.1 / 4.2.2).

Spec: deploy/04-kb-distribution.md 4.2.2 defines the version hash EXACTLY as:

    kb_hash = sha256( sorted( file_path + sha256(file_content) for file in content/ ) )

This script implements that recipe byte-for-byte over a KB `content/` tree, emitting:
  - checksums.txt  (sha256sum format: "<64hex>  <relpath>", sorted) -- consumed by
    crates/kb git_pull::parse_checksums / verify_checksums (KBC-04).
  - manifest.json  with the aggregate global_hash -- consumed by crates/kb
    manifest::verify_global_hash (KBC-02 / INV-04 freeze anchor).

The aggregate matches crates/kb manifest::compute_global_hash so the client and publisher agree
bit-for-bit (cross-checked by tests/kb_manifest_parity in the kb crate's verify_checkout tests).
The global_hash is derived from real file bytes, never invented (no-mock constraint).

Pure standard library. Zero Emoji. Cross-machine.

Usage:
  python gen_kb_manifest.py --content <content-dir> --out <release-dir> --version 2026-07-01
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


def collect_file_hashes(content_dir: str) -> list:
    """Return a sorted list of (relpath_with_forward_slashes, sha256) over every file."""
    entries = []
    for root, _dirs, files in os.walk(content_dir):
        for fn in files:
            full = os.path.join(root, fn)
            rel = os.path.relpath(full, content_dir).replace(os.sep, "/")
            entries.append((rel, sha256_file(full)))
    entries.sort(key=lambda e: e[0])
    return entries


def compute_global_hash(entries: list) -> str:
    """Match crates/kb manifest::compute_global_hash BIT-FOR-BIT (deploy/04 4.2.2).

    Rust recipe: each pair contributes the line "<path>\\u{1F}<file_hash>" (U+001F unit
    separator); the lines are sorted as full strings; then each line, followed by a "\\n",
    is fed into one running SHA-256. We replicate exactly so the publisher and the client
    (crates/kb verify_manifest_global_hash) agree.
    """
    lines = ["%s\x1f%s" % (path, h) for (path, h) in entries]
    lines.sort()
    agg = hashlib.sha256()
    for line in lines:
        agg.update(line.encode("utf-8"))
        agg.update(b"\n")
    return agg.hexdigest()


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description="Generate KB checksums.txt + manifest.json (deploy/04 4.2.2).")
    parser.add_argument("--content", required=True, help="KB content/ directory.")
    parser.add_argument("--out", required=True, help="Output release directory.")
    parser.add_argument("--version", required=True, help="Version label, e.g. 2026-07-01.")
    parser.add_argument("--schema-version", type=int, default=1)
    args = parser.parse_args(argv)

    if not os.path.isdir(args.content):
        sys.stderr.write("content dir not found: %s\n" % args.content)
        return 2

    entries = collect_file_hashes(args.content)
    if not entries:
        sys.stderr.write("no files under content dir: %s\n" % args.content)
        return 2

    global_hash = compute_global_hash(entries)
    os.makedirs(args.out, exist_ok=True)

    checksums_path = os.path.join(args.out, "checksums.txt")
    with open(checksums_path, "w", encoding="utf-8", newline="\n") as fh:
        for rel, h in entries:
            fh.write("%s  %s\n" % (h, rel))

    manifest = {
        "version": args.version,
        "global_hash": global_hash,
        "schema_version": args.schema_version,
        "file_count": len(entries),
        "format_version": "1.0",
    }
    manifest_path = os.path.join(args.out, "manifest.json")
    with open(manifest_path, "w", encoding="utf-8", newline="\n") as fh:
        json.dump(manifest, fh, ensure_ascii=False, indent=2)
        fh.write("\n")

    sys.stdout.write("KB manifest generated:\n")
    sys.stdout.write("  files       : %d\n" % len(entries))
    sys.stdout.write("  global_hash : %s\n" % global_hash)
    sys.stdout.write("  checksums   : %s\n" % checksums_path)
    sys.stdout.write("  manifest    : %s\n" % manifest_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
