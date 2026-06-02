#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""R1 CI gate: assert the README mandatory first block is byte-for-byte correct.

Spec: prompts/0529/spec/compliance/04-gplv3-readme-front.md 2.1 and 8;
prd/05-non-functional.md 5.3.4. This is the INV-09 engineering contract: the
mandatory acknowledgement wording must be the very first content of README.md,
before any banner, badge, or heading, with no folding and no language toggle.

The check is strict and byte-oriented:
  - The first N characters of README.md (after stripping a UTF-8 BOM if present,
    which some Windows editors prepend) must equal MANDATORY_FIRST_BLOCK exactly,
    including every punctuation mark and every newline inside the block.
  - The only normalisation permitted by the spec is the trailing newline: the
    block may be followed by a newline (or EOF). Nothing else is normalised.

Exit code 0 on match. Exit code 1 with a line-level diff on mismatch.

Pure standard library. Zero Emoji. Runs on any machine and OS.

Usage:
  python tools/qa/check_readme_first_line.py
  python tools/qa/check_readme_first_line.py --readme path/to/README.md
"""

from __future__ import annotations

import argparse
import difflib
import os
import sys

_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.abspath(os.path.join(_THIS_DIR, os.pardir, os.pardir))
DEFAULT_README = os.path.join(PROJECT_ROOT, "README.md")

# Verbatim mandatory first block. Source of truth: spec 2.1 / prd 5.3.4.
# Do not "fix", reflow, or translate any character here.
MANDATORY_FIRST_BLOCK = (
    "本许可证（GPLv3）允许任何人 fork、修改、再分发本项目，\n"
    "项目方对反向用途（如雇主版分支）无法做技术防御。\n"
    "反 HR 立场是道德姿态，请用户与开发者基于共同立场使用本项目。"
)

_BOM = "﻿"


def read_readme(path: str) -> str:
    if not os.path.exists(path):
        raise FileNotFoundError(path)
    # newline="" preserves the file's exact newline bytes so the byte-for-byte
    # comparison is honest; we do not let Python translate CRLF.
    with open(path, "r", encoding="utf-8", newline="") as fh:
        return fh.read()


def check(text: str) -> tuple[bool, str]:
    """Return (ok, message). On failure, message contains a unified diff."""
    content = text[len(_BOM):] if text.startswith(_BOM) else text

    expected = MANDATORY_FIRST_BLOCK
    actual_prefix = content[: len(expected)]

    if actual_prefix == expected:
        # Enforce the "first content" requirement: only a newline (the permitted
        # trailing-newline normalisation) or EOF may follow the block. Any other
        # character means the block is glued to a heading/banner with no break,
        # which violates spec 2.1 ("before any banner / badge").
        trailing = content[len(expected): len(expected) + 1]
        if trailing in ("", "\n", "\r"):
            return True, "OK: README mandatory first block matches byte-for-byte."
        return (
            False,
            "FAIL: mandatory block is present but not terminated by a newline.\n"
            "      The block must be the first content and be followed by a line break.\n"
            "      Got the character {!r} immediately after the block.".format(trailing),
        )

    expected_lines = expected.splitlines(keepends=True)
    # Compare against the same number of source lines for a readable diff.
    actual_lines = content.splitlines(keepends=True)[: len(expected_lines)]
    diff = "".join(
        difflib.unified_diff(
            expected_lines,
            actual_lines,
            fromfile="EXPECTED (spec 2.1)",
            tofile="ACTUAL (README.md head)",
        )
    )
    return (
        False,
        "FAIL: README first block does not match the mandatory wording.\n" + diff,
    )


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        description="Assert README mandatory first block byte-for-byte (R1 CI gate)."
    )
    parser.add_argument("--readme", default=DEFAULT_README, help="README path.")
    args = parser.parse_args(argv)

    try:
        text = read_readme(args.readme)
    except FileNotFoundError:
        sys.stderr.write(
            "FAIL: README not found at " + args.readme + "\n"
        )
        return 1

    ok, message = check(text)
    if ok:
        sys.stdout.write(message + "\n")
        return 0
    sys.stderr.write(message + "\n")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
