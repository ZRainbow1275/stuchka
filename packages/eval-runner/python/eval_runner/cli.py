"""Run every eval gate, print the summary, exit non-zero on any required-gate miss (ai/05 §5.10).

Required automated gates: calc-50, deadline-30, doc-20 (GB45438), pii-200, law-200 (structural).
Human process gates (doc usability, law accuracy, fact-30) and the R1b-deferred abstention-300 are
reported but never block. Usage: python -m eval_runner.cli [--release]
"""
from __future__ import annotations

import sys

from . import (
    run_abstention_300,
    run_calc_50,
    run_deadline_30,
    run_doc_20,
    run_fact_30,
    run_law_200,
    run_pii_200,
)
from .common import GateOutcome, Status, render_summary


def _safe(name: str, fn, release: bool) -> GateOutcome:
    try:
        return fn(release=release)
    except Exception as e:  # a required gate that cannot even run is a blocking failure
        return GateOutcome(name, Status.FAIL, True, f"error: {e}")


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    release = "--release" in argv

    outcomes: list[GateOutcome] = [
        _safe("calc-50", run_calc_50.run, release),
        _safe("deadline-30", run_deadline_30.run, release),
        _safe("doc-20 (GB45438)", run_doc_20.run, release),
        _safe("pii-200", run_pii_200.run, release),
        _safe("law-200 (structural)", run_law_200.run, release),
        # Human process gates + R1b-deferred (reported, non-blocking):
        run_doc_20.usability_process(),
        run_law_200.accuracy_process(),
        run_fact_30.run(),
        run_abstention_300.run(),
    ]
    ok = render_summary(outcomes)
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
