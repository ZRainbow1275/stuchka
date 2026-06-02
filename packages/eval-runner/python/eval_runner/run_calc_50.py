"""calc-50: M9 计算正确率 == 100% (ai/05 §5.3, 错一道不发版). AUTOMATED, REQUIRED."""
from . import backend, thresholds
from .common import GateOutcome, Status


def run(release: bool = False) -> GateOutcome:
    r = backend.run_oracle("calc", release=release)
    ok = r.pass_rate >= thresholds.CALC_50_PASS_RATE_MIN
    detail = f"pass_rate={r.pass_rate:.4f} ({r.passed}/{r.total}) [>= {thresholds.CALC_50_PASS_RATE_MIN}]"
    if not ok:
        detail += f" FAIL: {r.failing_ids()}"
    return GateOutcome("calc-50", Status.PASS if ok else Status.FAIL, True, detail)
