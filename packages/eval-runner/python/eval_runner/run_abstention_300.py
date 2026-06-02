"""abstention-300: 拒答率 <= 10% + 桶命中 >= 90% (ai/05 §5.6, ai/03 §3.7-§3.8). AUTOMATED, REQUIRED.

Drives the REAL `ai_dispatcher` INV-08 three-tier path (compute_confidence -> bucket -> compose) +
the genuine refusal answers via the Rust oracle (`eval-cli abstention`). The oracle reports two
metrics; this runner decides them against thresholds.py. The human path-runner (5 volunteers + the
running app, ai/05 §5.6) remains a separate process seam (datasets/path-runner) — never auto-passed.
"""
from . import backend, thresholds
from .common import GateOutcome, Status


def run(release: bool = False) -> GateOutcome:
    r = backend.run_oracle("abstention", release=release)
    bucket_hit = float(r.metrics.get("bucket_hit", 0.0))
    refusal_rate = float(r.metrics.get("refusal_rate", 1.0))
    ok = (
        refusal_rate <= thresholds.ABSTENTION_REFUSAL_RATE_MAX
        and bucket_hit >= thresholds.ABSTENTION_BUCKET_HIT_MIN
    )
    detail = (
        f"refusal_rate={refusal_rate:.4f} [<= {thresholds.ABSTENTION_REFUSAL_RATE_MAX}] "
        f"bucket_hit={bucket_hit:.4f} [>= {thresholds.ABSTENTION_BUCKET_HIT_MIN}] "
        f"({r.passed}/{r.total} cases)"
    )
    if not ok:
        detail += f" FAIL: {r.failing_ids()[:10]}"
    return GateOutcome("abstention-300", Status.PASS if ok else Status.FAIL, True, detail)
