#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""L0 public-binary-release guard.

Spec: prompts/0529/spec/compliance/03-filing-exemption-stub.md 5 +
prompts/0529/prd/05-non-functional.md 5.3.3 (L0-02 filing exemption) and
5.3.5 (L0-01 legal subject).

Real engineering rule, NOT a placeholder:
  Publishing a PUBLIC BINARY (GitHub Release, packaged installer) is permitted
  ONLY when BOTH real-world legal preconditions hold:
    L0-02  legal/opinions/latest.yaml  verdict == 'exempt'  AND not expired.
    L0-01  legal/subject/current.yaml  type is a legal entity (not 'individual')
           AND confirmed == true.
  Otherwise the project ships SOURCE ONLY and this guard exits non-zero so a
  release pipeline cannot publish binaries ahead of the legal status.

The current repository state (verdict 'none', individual maintainer) is the real
unresolved L0-01/L0-02 situation, so this guard is expected to BLOCK today. That
is correct behaviour, not a failure of this script.

Design constraints (task hard constraints):
  - Pure standard library. PyYAML used if importable, else a tiny flat-mapping
    parser handles the intentionally-flat YAML. Runs on any machine / OS.
  - Zero Emoji anywhere in this file or its output.
  - No mock data: the decision is derived from the real legal YAML metadata.

Usage:
  python tools/release/check_l0_guard.py            # exit 0 if binary release allowed, else 1
  python tools/release/check_l0_guard.py --json      # machine-readable verdict, same exit code
  python tools/release/check_l0_guard.py --opinion P --subject P
"""

from __future__ import annotations

import argparse
import json
import os
import sys

_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.abspath(os.path.join(_THIS_DIR, os.pardir, os.pardir))
DEFAULT_OPINION = os.path.join(PROJECT_ROOT, "legal", "opinions", "latest.yaml")
DEFAULT_SUBJECT = os.path.join(PROJECT_ROOT, "legal", "subject", "current.yaml")

# Legal-entity subject types that satisfy L0-01 (prd 5.3.5). An individual
# maintainer never unlocks public binaries.
LEGAL_ENTITY_TYPES = ("nonprofit_foundation", "company", "foundation", "corporation")


# ---------------------------------------------------------------------------
# YAML loading: PyYAML when present, minimal flat parser otherwise.
# (Kept self-contained so the guard has no cross-file import fragility.)
# ---------------------------------------------------------------------------

def _strip_inline_comment(value: str) -> str:
    in_single = False
    in_double = False
    for i, ch in enumerate(value):
        if ch == "'" and not in_double:
            in_single = not in_single
        elif ch == '"' and not in_single:
            in_double = not in_double
        elif ch == "#" and not in_single and not in_double:
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
    if len(value) >= 2 and value[0] == value[-1] and value[0] in ("'", '"'):
        return value[1:-1]
    try:
        return int(value)
    except ValueError:
        return value


def _minimal_yaml_load(text: str) -> dict:
    result: dict = {}
    for raw in text.splitlines():
        line = raw.rstrip("\n").rstrip("\r")
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if ":" not in line:
            continue
        key, _, value = line.partition(":")
        key = key.strip()
        if not key:
            continue
        result[key] = _coerce_scalar(_strip_inline_comment(value).strip())
    return result


def load_yaml(path: str) -> dict:
    if not os.path.exists(path):
        return {}
    with open(path, "r", encoding="utf-8") as fh:
        text = fh.read()
    try:
        import yaml  # type: ignore

        data = yaml.safe_load(text)
        return data if isinstance(data, dict) else {}
    except ImportError:
        return _minimal_yaml_load(text)


# ---------------------------------------------------------------------------
# Decision
# ---------------------------------------------------------------------------

def _today_iso() -> str:
    # Deferred import; only used for the optional expiry comparison.
    import datetime

    return datetime.date.today().isoformat()


def evaluate(opinion: dict, subject: dict) -> dict:
    """Return a decision mapping. allow_binary is True only if every gate passes."""
    reasons: list[str] = []

    verdict = opinion.get("verdict")
    verdict = ("" if verdict is None else str(verdict)).strip().lower()
    expires_at = opinion.get("expires_at")
    expires_at = "" if expires_at is None else str(expires_at).strip()

    subj_type = subject.get("type")
    subj_type = ("" if subj_type is None else str(subj_type)).strip().lower()
    confirmed = subject.get("confirmed")
    confirmed = bool(confirmed) if isinstance(confirmed, bool) else str(confirmed).strip().lower() == "true"

    # L0-02 gate.
    l0_02_ok = verdict == "exempt"
    if not l0_02_ok:
        if verdict in ("", "none"):
            reasons.append("L0-02: 律所备案豁免法律意见书尚未出具 (verdict=none)。")
        else:
            reasons.append("L0-02: 律所意见 verdict=%s 非 exempt，公开二进制未解锁。" % verdict)
    elif expires_at and expires_at < _today_iso():
        l0_02_ok = False
        reasons.append("L0-02: 律所意见已于 %s 失效，需重新出具。" % expires_at)

    # L0-01 gate.
    l0_01_ok = subj_type in LEGAL_ENTITY_TYPES and confirmed
    if subj_type not in LEGAL_ENTITY_TYPES:
        reasons.append(
            "L0-01: 法律责任主体为 '%s'，非法人主体；个人开发者不发布公开二进制 (prd 5.3.5)。"
            % (subj_type or "未设置")
        )
    elif not confirmed:
        reasons.append("L0-01: 法人主体登记尚未确认 (confirmed=false)。")

    allow_binary = l0_02_ok and l0_01_ok
    return {
        "allow_binary": allow_binary,
        "allow_source": True,  # source-only release is ALWAYS permitted under GPLv3.
        "l0_01_ok": l0_01_ok,
        "l0_02_ok": l0_02_ok,
        "verdict": verdict or "none",
        "subject_type": subj_type or "unset",
        "reasons": reasons,
    }


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        description="Gate public binary release on the real L0-01/L0-02 legal status."
    )
    parser.add_argument("--opinion", default=DEFAULT_OPINION, help="latest.yaml path.")
    parser.add_argument("--subject", default=DEFAULT_SUBJECT, help="current.yaml path.")
    parser.add_argument("--json", action="store_true", help="Machine-readable output.")
    args = parser.parse_args(argv)

    decision = evaluate(load_yaml(args.opinion), load_yaml(args.subject))

    if args.json:
        sys.stdout.write(json.dumps(decision, ensure_ascii=False, indent=2) + "\n")
    else:
        if decision["allow_binary"]:
            sys.stdout.write(
                "check_l0_guard: OK — 公开二进制发布已解锁 "
                "(L0-01 法人主体已确认, L0-02 verdict=exempt)。\n"
            )
        else:
            sys.stdout.write("check_l0_guard: BLOCK — 公开二进制发布被阻断；仅允许源码发布。\n")
            for reason in decision["reasons"]:
                sys.stdout.write("  - " + reason + "\n")
            sys.stdout.write(
                "  自编译模式: 用户可从源码自行编译 (见 README 构建说明)。\n"
            )

    return 0 if decision["allow_binary"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
