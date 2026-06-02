"""law-200: structural URN validity == 100% (ai/05 §5.2). AUTOMATED, REQUIRED.

The 法条准确率 >= 95% lawyer-blind layer is a HUMAN process gate (datasets/law-200/README.md), reported
separately and never auto-passed.
"""
from . import backend, thresholds
from .common import GateOutcome, Status


def run(release: bool = False) -> GateOutcome:
    r = backend.run_oracle("law", release=release)
    ok = r.pass_rate >= thresholds.LAW_200_STRUCTURAL_MIN
    detail = (
        f"urn_structural_valid={r.pass_rate:.4f} ({r.passed}/{r.total}) "
        f"[>= {thresholds.LAW_200_STRUCTURAL_MIN}]"
    )
    if not ok:
        detail += f" FAIL: {r.failing_ids()}"
    return GateOutcome("law-200 (structural)", Status.PASS if ok else Status.FAIL, True, detail)


def accuracy_process() -> GateOutcome:
    return GateOutcome(
        "law-200 (accuracy)",
        Status.PROCESS,
        False,
        f"法条准确率 >= {thresholds.LAW_200_ACCURACY_MIN} is a 2-lawyer+1-lecturer blind HUMAN gate (§5.9)",
    )
