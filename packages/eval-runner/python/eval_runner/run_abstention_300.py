"""abstention-300: 拒答率 <= 10% + 桶命中 >= 90% (ai/05 §5.6). DEFERRED to R1b (§5.12)."""
from . import thresholds
from .common import GateOutcome, Status


def run(release: bool = False) -> GateOutcome:
    return GateOutcome(
        "abstention-300",
        Status.DEFERRED,
        False,
        f"拒答率 <= {thresholds.ABSTENTION_REFUSAL_RATE_MAX} deferred to R1b (datasets/abstention-300/README.md)",
    )
