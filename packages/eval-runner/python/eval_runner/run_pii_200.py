"""pii-200: 高敏检测 召回 >= 95% / 误报 <= 5% (ai/05 §5.7 + ai/04 §4.7). AUTOMATED, REQUIRED."""
from . import backend, thresholds
from .common import GateOutcome, Status


def run(release: bool = False) -> GateOutcome:
    r = backend.run_oracle("pii", release=release)
    recall = float(r.metrics.get("recall", 0.0))
    fp_rate = float(r.metrics.get("fp_rate", 1.0))
    ok = recall >= thresholds.PII_200_RECALL_MIN and fp_rate <= thresholds.PII_200_FP_RATE_MAX
    detail = (
        f"recall={recall:.4f} [>= {thresholds.PII_200_RECALL_MIN}], "
        f"fp_rate={fp_rate:.4f} [<= {thresholds.PII_200_FP_RATE_MAX}] "
        f"(n={r.total})"
    )
    if not ok:
        detail += f" FAIL: {r.failing_ids()[:10]}"
    return GateOutcome("pii-200", Status.PASS if ok else Status.FAIL, True, detail)
