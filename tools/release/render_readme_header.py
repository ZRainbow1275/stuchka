#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Render the README dynamic second line from legal/opinions/latest.yaml.

Spec: prompts/0529/spec/compliance/04-gplv3-readme-front.md 2.2 (verdict table).

The README first block (mandatory INV-09 wording) is fixed and never touched by
this script. This script only (re)writes the single line that immediately follows
that block, driven by the lawyer-opinion verdict, so that the published document
always agrees with the real legal status.

Design constraints (task hard constraints):
  - Pure standard library. PyYAML is used if importable, otherwise a tiny
    flat-mapping YAML parser handles latest.yaml (which is intentionally a flat
    scalar mapping). Runs on any machine, any OS, no third-party requirement.
  - Idempotent: running it twice in a row produces a byte-identical README.
  - Zero Emoji anywhere in this file or its output.
  - No mock data: the wording comes verbatim from the spec verdict table; when no
    opinion exists yet (verdict none / missing) the truthful "not yet issued"
    state is rendered, which is the real current legal situation.

Usage:
  python tools/release/render_readme_header.py            # rewrite README.md in place
  python tools/release/render_readme_header.py --check     # exit 1 if README would change
  python tools/release/render_readme_header.py --print     # print only the rendered line
  python tools/release/render_readme_header.py --readme PATH --opinion PATH
"""

from __future__ import annotations

import argparse
import os
import sys

# ----------------------------------------------------------------------------
# Paths
# ----------------------------------------------------------------------------

# This file lives at <root>/tools/release/render_readme_header.py.
_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.abspath(os.path.join(_THIS_DIR, os.pardir, os.pardir))
DEFAULT_README = os.path.join(PROJECT_ROOT, "README.md")
DEFAULT_OPINION = os.path.join(PROJECT_ROOT, "legal", "opinions", "latest.yaml")

# ----------------------------------------------------------------------------
# Mandatory first block (spec 2.1). Must equal check_readme_first_line.py.
# Kept here so this script can locate where the dynamic line goes, and so it can
# scaffold a minimal README if one does not exist yet.
# ----------------------------------------------------------------------------

MANDATORY_FIRST_BLOCK = (
    "本许可证（GPLv3）允许任何人 fork、修改、再分发本项目，\n"
    "项目方对反向用途（如雇主版分支）无法做技术防御。\n"
    "反 HR 立场是道德姿态，请用户与开发者基于共同立场使用本项目。"
)

# A stable machine-readable marker that brackets the rendered line. The render
# target is the text BETWEEN these two HTML comments. Comments are invisible in
# rendered Markdown and survive round-trips, which keeps the operation idempotent
# and robust to manual edits elsewhere in the file.
RENDER_BEGIN = "<!-- RENDERED_LEGAL_STATUS_LINE:BEGIN -->"
RENDER_END = "<!-- RENDERED_LEGAL_STATUS_LINE:END -->"


# ----------------------------------------------------------------------------
# Verdict -> second line (spec 2.2 table, verbatim)
# ----------------------------------------------------------------------------

def render_second_line(opinion: dict) -> str:
    """Return the README second line for the given opinion mapping.

    Mirrors the spec 2.2 verdict table exactly. The "未出具" (not yet issued)
    branch is the default and covers both verdict == 'none' and a missing /
    empty verdict, which is the real current state of this project.
    """
    verdict = opinion.get("verdict")
    verdict = ("" if verdict is None else str(verdict)).strip().lower()

    if verdict == "exempt":
        sha = opinion.get("sha256")
        sha = "" if sha is None else str(sha).strip()
        sha12 = sha[:12] if sha else "（待回填）"
        firm = opinion.get("law_firm") or opinion.get("firm") or "[律所]"
        firm = str(firm).strip() or "[律所]"
        return (
            "备案豁免假设由 {firm} 出具的法律意见书（SHA-256: {sha12}...）支持。"
            "详见 LICENSE 附录 C。".format(firm=firm, sha12=sha12)
        )

    if verdict == "filing_required":
        return (
            "本项目暂以源代码形式发布，公开二进制下架。"
            "详见 LICENSE 附录 C 与“自编译模式说明”。"
        )

    if verdict == "ambiguous":
        return (
            "本项目暂以源代码形式发布，等待律所二次意见。"
            "详见 LICENSE 附录 C。"
        )

    # verdict in {"none", ""} or unrecognised -> truthful not-yet-issued state.
    return "本项目当前由个人维护，律所意见书尚未出具，公开二进制发布暂停。"


# ----------------------------------------------------------------------------
# YAML loading: PyYAML when present, minimal flat parser otherwise.
# ----------------------------------------------------------------------------

def _minimal_yaml_load(text: str) -> dict:
    """Parse a flat scalar mapping. Sufficient for legal/opinions/latest.yaml.

    Supports: 'key: value' lines, '#' comments, blank lines, and the scalars
    null/~/'' -> None, true/false -> bool, ints, and quoted/unquoted strings.
    Does NOT support nested mappings or lists (latest.yaml is intentionally flat).
    """
    result: dict = {}
    for raw in text.splitlines():
        line = raw.rstrip("\n").rstrip("\r")
        # Strip full-line comments and blanks.
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if ":" not in line:
            continue
        key, _, value = line.partition(":")
        key = key.strip()
        if not key:
            continue
        # Remove inline comments that are not inside quotes.
        value = _strip_inline_comment(value).strip()
        result[key] = _coerce_scalar(value)
    return result


def _strip_inline_comment(value: str) -> str:
    in_single = False
    in_double = False
    for i, ch in enumerate(value):
        if ch == "'" and not in_double:
            in_single = not in_single
        elif ch == '"' and not in_single:
            in_double = not in_double
        elif ch == "#" and not in_single and not in_double:
            # A comment must be preceded by whitespace to count (YAML rule).
            if i == 0 or value[i - 1] in " \t":
                return value[:i]
    return value


def _coerce_scalar(value: str):
    if value == "":
        return None
    low = value.lower()
    if low in ("null", "~"):
        return None
    if low == "true":
        return True
    if low == "false":
        return False
    # Quoted string -> unwrap.
    if len(value) >= 2 and value[0] == value[-1] and value[0] in ("'", '"'):
        return value[1:-1]
    # Integer.
    try:
        return int(value)
    except ValueError:
        return value


def load_opinion(path: str) -> dict:
    if not os.path.exists(path):
        # Missing file is a legitimate "not yet issued" state, not an error.
        return {"verdict": "none"}
    with open(path, "r", encoding="utf-8") as fh:
        text = fh.read()
    try:
        import yaml  # type: ignore

        data = yaml.safe_load(text)
        if isinstance(data, dict):
            return data
        return {"verdict": "none"}
    except ImportError:
        return _minimal_yaml_load(text)


# ----------------------------------------------------------------------------
# README assembly
# ----------------------------------------------------------------------------

def _scaffold_readme(second_line: str) -> str:
    """Build a minimal valid README when none exists, with the mandatory block
    first and the rendered line between the markers."""
    return (
        MANDATORY_FIRST_BLOCK
        + "\n\n"
        + RENDER_BEGIN
        + "\n"
        + second_line
        + "\n"
        + RENDER_END
        + "\n"
    )


def apply_to_readme(readme_text: str, second_line: str) -> str:
    """Return README text with the rendered line set to second_line.

    If the markers are present, replace only the content between them. The
    operation is idempotent. If the markers are absent, insert them immediately
    after the mandatory first block.
    """
    begin = readme_text.find(RENDER_BEGIN)
    end = readme_text.find(RENDER_END)
    block = RENDER_BEGIN + "\n" + second_line + "\n" + RENDER_END

    if begin != -1 and end != -1 and end > begin:
        end_full = end + len(RENDER_END)
        return readme_text[:begin] + block + readme_text[end_full:]

    # Markers absent: insert after the mandatory first block if found.
    idx = readme_text.find(MANDATORY_FIRST_BLOCK)
    if idx != -1:
        after = idx + len(MANDATORY_FIRST_BLOCK)
        # Normalise: exactly one blank line between block and rendered region.
        tail = readme_text[after:]
        return (
            readme_text[:after]
            + "\n\n"
            + block
            + ("\n" + tail.lstrip("\n") if tail.strip() else "\n")
        )

    # No mandatory block at all: prepend a fresh scaffold (defensive).
    return _scaffold_readme(second_line) + "\n" + readme_text


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        description="Render README dynamic legal-status line from latest.yaml."
    )
    parser.add_argument("--readme", default=DEFAULT_README, help="README path.")
    parser.add_argument("--opinion", default=DEFAULT_OPINION, help="latest.yaml path.")
    parser.add_argument(
        "--check",
        action="store_true",
        help="Do not write. Exit 1 if README would change, else 0.",
    )
    parser.add_argument(
        "--print",
        dest="print_only",
        action="store_true",
        help="Print only the rendered line and exit 0.",
    )
    args = parser.parse_args(argv)

    opinion = load_opinion(args.opinion)
    second_line = render_second_line(opinion)

    if args.print_only:
        sys.stdout.write(second_line + "\n")
        return 0

    if os.path.exists(args.readme):
        with open(args.readme, "r", encoding="utf-8", newline="") as fh:
            current = fh.read()
        updated = apply_to_readme(current, second_line)
    else:
        current = None
        updated = _scaffold_readme(second_line)

    if args.check:
        if current is None or current != updated:
            sys.stderr.write(
                "render_readme_header: README is out of date with "
                + os.path.relpath(args.opinion, PROJECT_ROOT)
                + ". Run: python tools/release/render_readme_header.py\n"
            )
            return 1
        sys.stdout.write("render_readme_header: README legal-status line is current.\n")
        return 0

    if current == updated:
        sys.stdout.write(
            "render_readme_header: no change (rendered line: " + second_line + ")\n"
        )
        return 0

    with open(args.readme, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(updated)
    sys.stdout.write(
        "render_readme_header: README updated (rendered line: " + second_line + ")\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
