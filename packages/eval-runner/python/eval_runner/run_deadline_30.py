"""deadline-30: M5 时效正确率 == 100% (ai/05 §5.3 + 06 §6.9, 错一道不发版). AUTOMATED, REQUIRED."""
from . import backend, thresholds
from .common import GateOutcome, Status


def run(release: bool = False) -> GateOutcome:
    r = backend.run_oracle("deadline", release=release)
    ok = r.pass_rate >= thresholds.DEADLINE_30_PASS_RATE_MIN
    detail = f"pass_rate={r.pass_rate:.4f} ({r.passed}/{r.total}) [>= {thresholds.DEADLINE_30_PASS_RATE_MIN}]"
    if not ok:
        detail += f" FAIL: {r.failing_ids()}"
    return GateOutcome("deadline-30", Status.PASS if ok else Status.FAIL, True, detail)
