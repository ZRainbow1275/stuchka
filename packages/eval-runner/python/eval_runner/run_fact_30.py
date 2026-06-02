"""fact-30: 事实核验率 >= 80% (ai/05 §5.5). HUMAN PROCESS GATE — reported, not auto-decided."""
from . import thresholds
from .common import GateOutcome, Status


def run(release: bool = False) -> GateOutcome:
    return GateOutcome(
        "fact-30",
        Status.PROCESS,
        False,
        f"事实核验率 >= {thresholds.FACT_30_VERIFIED_RATE_MIN} is a 5-reviewer HUMAN gate (datasets/fact-30/README.md)",
    )
