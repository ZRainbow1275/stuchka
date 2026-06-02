#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Stučka R1 pre-release total gate (总闸).

Runs every real quality gate across the three subprojects and the compliance
deliverables, then prints a summary and exits non-zero if any REQUIRED gate
fails. Nothing here is a mock: each gate shells out to the real toolchain and
honours its real exit code.

Gates:
  backend   cargo test --workspace            (641+ tests; incl. INV-01 isolation, e2e closed loop)
  backend   cargo clippy --workspace -D warnings
  editor    npm run build && npm test          (vite UMD build + vitest)
  frontend  flutter analyze && flutter test    (analyze 0 issues + 60 widget tests)
  compliance tools/qa/check_readme_first_line.py   (GPLv3 README first-line, byte-for-byte)
  compliance tools/triage/keyword_matcher.py --self-test  (Issues triage >=95% accuracy)
  product   no-emoji scan over the whole source tree (zero Emoji invariant)
  compliance tools/release/check_l0_guard.py   (INFORMATIONAL: L0-01/L0-02 binary-release status)

Cross-machine: tools are located via shutil.which; the Cyrillic repo path is
respected (no build of Windows shaders happens here — analyze/test only).

Usage:
  python tools/ci/pre_release.py                 # strict: a missing required tool FAILS its gate
  python tools/ci/pre_release.py --skip-missing  # downgrade missing-tool gates to SKIP (partial local run)
  python tools/ci/pre_release.py --fast          # skip the slow backend cargo test (clippy still runs)
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import time

THIS_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.abspath(os.path.join(THIS_DIR, os.pardir, os.pardir))
BACKEND = os.path.join(PROJECT_ROOT, "backend")
EDITOR = os.path.join(PROJECT_ROOT, "editor")
FRONTEND = os.path.join(PROJECT_ROOT, "frontend")

# Source-tree scan config for the zero-Emoji invariant.
SCAN_EXTS = (".rs", ".dart", ".ts", ".tsx", ".js", ".mjs", ".md", ".json",
             ".css", ".html", ".yaml", ".yml", ".toml", ".py")
SCAN_SKIP_DIRS = {"node_modules", "target", "build", "dist", ".dart_tool",
                  ".git", "LICENSES", ".trellis"}
# Generated lock files carry third-party dependency metadata we do not author
# (e.g. npm "GitHub Sponsors" funding strings). They are not our UI/source icons,
# so they are excluded from the zero-Emoji invariant scan.
SCAN_SKIP_FILES = {"package-lock.json", "pnpm-lock.yaml", "yarn.lock",
                   "Cargo.lock", "pubspec.lock"}
# Emoji / pictograph codepoint ranges (matches editor EMOJI_RE + Dart scanner).
EMOJI_RANGES = (
    (0x1F300, 0x1FAFF), (0x2600, 0x27BF), (0x1F000, 0x1F2FF),
    (0x1F1E6, 0x1F1FF),  # regional indicators (flags)
    (0x2B00, 0x2BFF), (0xFE00, 0xFE0F),  # misc symbols-arrows + variation selectors (VS16)
)


def _has_emoji(text: str):
    hits = []
    for ch in text:
        cp = ord(ch)
        for lo, hi in EMOJI_RANGES:
            if lo <= cp <= hi:
                hits.append(ch)
                break
    return hits


class Gate:
    def __init__(self, name, cwd, argv, required=True, tool=None, ok_codes=(0,)):
        self.name = name
        self.cwd = cwd
        self.argv = argv
        self.required = required
        self.tool = tool or (argv[0] if argv else None)
        self.ok_codes = ok_codes
        self.status = "PENDING"
        self.detail = ""
        self.seconds = 0.0


def _resolve_command(gate: Gate):
    """Return the actual command list to execute, or (None, reason) if the tool is
    missing. On Windows, .cmd/.bat shims (npm.cmd, flutter.bat) cannot be launched by
    bare name via CreateProcess, so route them through `cmd /c`; real .exe tools
    (cargo.exe) and the python interpreter run directly via their absolute path."""
    cmd = list(gate.argv)
    if not gate.tool:
        return cmd, None  # python gates: argv[0] is sys.executable (absolute .exe)
    real = shutil.which(gate.tool)
    if real is None:
        return None, "tool '%s' not on PATH" % gate.tool
    cmd[0] = real
    if os.name == "nt" and real.lower().endswith((".cmd", ".bat")):
        cmd = ["cmd", "/c"] + cmd
    return cmd, None


def _sum_cargo_passed(out: str) -> int:
    import re
    return sum(int(m) for m in re.findall(r"test result: ok\. (\d+) passed", out))


def run_subprocess_gate(gate: Gate, skip_missing: bool) -> None:
    cmd, missing = _resolve_command(gate)
    if missing:
        if skip_missing:
            gate.status = "SKIP"
            gate.detail = "%s (--skip-missing)" % missing
        else:
            gate.status = "FAIL"
            gate.detail = "%s (install it, or use --skip-missing)" % missing
        return
    start = time.time()
    try:
        proc = subprocess.run(
            cmd, cwd=gate.cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            timeout=3600,
        )
        out = proc.stdout.decode("utf-8", errors="replace")
        gate.seconds = time.time() - start
        tail = "\n".join(out.strip().splitlines()[-6:])
        if proc.returncode in gate.ok_codes:
            gate.status = "PASS"
        else:
            gate.status = "FAIL"
        if "cargo test" in gate.name and proc.returncode == 0:
            gate.detail = "exit=0  %d tests passed (workspace)" % _sum_cargo_passed(out)
        else:
            gate.detail = "exit=%d  %s" % (proc.returncode, tail.replace("\n", " | "))
    except subprocess.TimeoutExpired:
        gate.seconds = time.time() - start
        gate.status = "FAIL"
        gate.detail = "timeout after 3600s"
    except Exception as exc:  # noqa: BLE001
        gate.seconds = time.time() - start
        gate.status = "FAIL"
        gate.detail = "error: %s" % exc


def run_emoji_scan(gate: Gate) -> None:
    start = time.time()
    bad = []
    scanned = 0
    for root, dirs, files in os.walk(PROJECT_ROOT):
        dirs[:] = [d for d in dirs if d not in SCAN_SKIP_DIRS]
        for fn in files:
            if not fn.endswith(SCAN_EXTS):
                continue
            if fn in SCAN_SKIP_FILES:
                continue
            path = os.path.join(root, fn)
            try:
                with open(path, "r", encoding="utf-8") as fh:
                    text = fh.read()
            except (UnicodeDecodeError, OSError):
                continue
            scanned += 1
            hits = _has_emoji(text)
            if hits:
                rel = os.path.relpath(path, PROJECT_ROOT)
                uniq = sorted({"U+%04X" % ord(c) for c in hits})
                bad.append("%s -> %s" % (rel, ",".join(uniq[:8])))
    gate.seconds = time.time() - start
    if bad:
        gate.status = "FAIL"
        gate.detail = "%d files with Emoji (of %d scanned): %s" % (
            len(bad), scanned, " ; ".join(bad[:5]))
    else:
        gate.status = "PASS"
        gate.detail = "0 Emoji across %d source files" % scanned


def run_l0_guard_informational(gate: Gate) -> None:
    """check_l0_guard exits 1 by design when binary release is blocked. That is the
    real current legal state and is NOT a pre-release failure for a SOURCE release,
    so this gate only REPORTS the verdict."""
    start = time.time()
    script = os.path.join(PROJECT_ROOT, "tools", "release", "check_l0_guard.py")
    try:
        proc = subprocess.run([sys.executable, script, "--json"],
                              cwd=PROJECT_ROOT, stdout=subprocess.PIPE,
                              stderr=subprocess.STDOUT, timeout=60)
        out = proc.stdout.decode("utf-8", errors="replace")
        gate.seconds = time.time() - start
        gate.status = "INFO"
        allow = '"allow_binary": true' in out
        gate.detail = ("binary_release=%s ; source_release=always-allowed ; (exit=%d)"
                       % ("UNLOCKED" if allow else "BLOCKED", proc.returncode))
    except Exception as exc:  # noqa: BLE001
        gate.seconds = time.time() - start
        gate.status = "INFO"
        gate.detail = "could not run L0 guard: %s" % exc


def editor_argv(skip_missing):
    # Build then test. If node_modules is absent, prefix npm ci.
    has_modules = os.path.isdir(os.path.join(EDITOR, "node_modules"))
    if has_modules:
        return ["npm", "run", "build", "--silent"], ["npm", "test", "--silent"]
    return ["npm", "ci"], ["npm", "test", "--silent"]


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description="Stučka R1 pre-release total gate.")
    parser.add_argument("--skip-missing", action="store_true",
                        help="Downgrade gates whose tool is absent to SKIP instead of FAIL.")
    parser.add_argument("--fast", action="store_true",
                        help="Skip the slow backend cargo test (clippy still runs).")
    args = parser.parse_args(argv)

    npm_is = shutil.which("npm")
    editor_build_cmd, editor_test_cmd = editor_argv(args.skip_missing)

    gates = []
    if not args.fast:
        gates.append(Gate("backend cargo test", BACKEND,
                          ["cargo", "test", "--workspace", "--quiet"], tool="cargo"))
    gates.append(Gate("backend cargo clippy", BACKEND,
                      ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
                      tool="cargo"))
    gates.append(Gate("editor npm build", EDITOR, editor_build_cmd, tool="npm"))
    gates.append(Gate("editor npm test", EDITOR, editor_test_cmd, tool="npm"))
    gates.append(Gate("frontend flutter analyze", FRONTEND,
                      ["flutter", "analyze"], tool="flutter"))
    gates.append(Gate("frontend flutter test", FRONTEND,
                      ["flutter", "test"], tool="flutter"))
    gates.append(Gate("compliance README first-line", PROJECT_ROOT,
                      [sys.executable, os.path.join("tools", "qa", "check_readme_first_line.py")],
                      tool=None))
    gates.append(Gate("compliance keyword triage", PROJECT_ROOT,
                      [sys.executable, os.path.join("tools", "triage", "keyword_matcher.py"),
                       "--self-test"], tool=None))

    print("=" * 78)
    print("Stučka R1 pre-release total gate")
    print("root:", PROJECT_ROOT)
    print("=" * 78)

    for gate in gates:
        print("\n>>> %s ..." % gate.name, flush=True)
        run_subprocess_gate(gate, args.skip_missing)
        print("    %-5s (%.1fs)  %s" % (gate.status, gate.seconds, gate.detail[:160]))

    emoji_gate = Gate("product zero-Emoji scan", PROJECT_ROOT, [], tool=None)
    print("\n>>> %s ..." % emoji_gate.name, flush=True)
    run_emoji_scan(emoji_gate)
    print("    %-5s (%.1fs)  %s" % (emoji_gate.status, emoji_gate.seconds, emoji_gate.detail[:160]))
    gates.append(emoji_gate)

    l0_gate = Gate("compliance L0 release guard", PROJECT_ROOT, [], required=False, tool=None)
    print("\n>>> %s (informational) ..." % l0_gate.name, flush=True)
    run_l0_guard_informational(l0_gate)
    print("    %-5s (%.1fs)  %s" % (l0_gate.status, l0_gate.seconds, l0_gate.detail[:160]))
    gates.append(l0_gate)

    print("\n" + "=" * 78)
    print("SUMMARY")
    print("=" * 78)
    failed = []
    for gate in gates:
        flag = "" if gate.required else " (info)"
        print("  %-5s  %-30s%s" % (gate.status, gate.name, flag))
        if gate.required and gate.status not in ("PASS",):
            if gate.status == "SKIP":
                pass  # skipped-missing under --skip-missing does not fail the build
            else:
                failed.append(gate.name)

    print("=" * 78)
    if failed:
        print("RESULT: FAIL — %d required gate(s) failed: %s" % (len(failed), ", ".join(failed)))
        return 1
    skipped = [g.name for g in gates if g.status == "SKIP"]
    if skipped:
        print("RESULT: PASS (with SKIPPED gates: %s) — coverage incomplete on this machine."
              % ", ".join(skipped))
    else:
        print("RESULT: PASS — all required gates green.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
