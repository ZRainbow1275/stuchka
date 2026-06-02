"""doc-20: GB45438 四层完整性 == 100% (ai/05 §5.4). AUTOMATED, REQUIRED.

The 文书可用率 >= 70% layer is a HUMAN process gate (see datasets/doc-20/README.md) and is reported
separately as a process gate, never auto-passed.
"""
from . import backend, thresholds
from .common import GateOutcome, Status


def run(release: bool = False) -> GateOutcome:
    r = backend.run_oracle("doc", release=release)
    ok = r.pass_rate >= thresholds.GB45438_COMPLETENESS_MIN
    detail = (
        f"gb45438_completeness={r.pass_rate:.4f} ({r.passed}/{r.total} docs) "
        f"[>= {thresholds.GB45438_COMPLETENESS_MIN}]"
    )
    if not ok:
        detail += f" FAIL: {r.failing_ids()}"
    return GateOutcome("doc-20 (GB45438)", Status.PASS if ok else Status.FAIL, True, detail)


def usability_process() -> GateOutcome:
    return GateOutcome(
        "doc-20 (usability)",
        Status.PROCESS,
        False,
        f"文书可用率 >= {thresholds.DOC_20_USABLE_RATE_MIN} is a 3-lawyer blind HUMAN gate (datasets/doc-20/README.md)",
    )
