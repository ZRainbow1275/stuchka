"""Shared models + reporting.

pydantic (ai/05 §5.1) is used for strict schema validation of the Rust oracle's report WHEN present;
absent, a stdlib structural check keeps the runner working anywhere. rich is used for the summary
table when present, else plain ASCII print (zero Emoji).
"""
from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Any

try:  # ai/05 §5.1 declared validation lib (soft dep)
    from pydantic import BaseModel, ConfigDict

    class _OracleReportModel(BaseModel):
        model_config = ConfigDict(extra="allow")
        gate: str
        total: int
        passed: int
        failed: int
        pass_rate: float
        metrics: dict[str, Any] = {}
        cases: list[dict[str, Any]] = []

    def _strict_validate(raw: dict[str, Any]) -> None:
        _OracleReportModel(**raw)  # raises pydantic.ValidationError on schema drift

    HAS_PYDANTIC = True
except Exception:  # pragma: no cover - stdlib fallback
    HAS_PYDANTIC = False

    def _strict_validate(raw: dict[str, Any]) -> None:
        required = ("gate", "total", "passed", "failed", "pass_rate", "cases")
        missing = [k for k in required if k not in raw]
        if missing:
            raise ValueError(f"oracle report missing keys: {missing}")
        if not isinstance(raw["cases"], list):
            raise ValueError("oracle report 'cases' must be a list")


class Status(str, Enum):
    PASS = "pass"
    FAIL = "fail"
    PROCESS = "process"        # human-graded; not auto-decided
    DEFERRED = "deferred_r1b"  # out of R1a scope


@dataclass
class GateReport:
    """The Rust oracle's per-gate result (validated)."""

    gate: str
    total: int
    passed: int
    failed: int
    pass_rate: float
    metrics: dict[str, Any] = field(default_factory=dict)
    cases: list[dict[str, Any]] = field(default_factory=list)

    @classmethod
    def from_dict(cls, raw: dict[str, Any]) -> "GateReport":
        _strict_validate(raw)
        return cls(
            gate=raw["gate"],
            total=int(raw["total"]),
            passed=int(raw["passed"]),
            failed=int(raw["failed"]),
            pass_rate=float(raw["pass_rate"]),
            metrics=dict(raw.get("metrics", {})),
            cases=list(raw.get("cases", [])),
        )

    def failing_ids(self) -> list[str]:
        return [c.get("id", "?") for c in self.cases if not c.get("pass", False)]


@dataclass
class GateOutcome:
    """A gate's decision against thresholds.py."""

    name: str
    status: Status
    required: bool
    detail: str

    @property
    def blocking_failure(self) -> bool:
        return self.required and self.status == Status.FAIL


def render_summary(outcomes: list[GateOutcome]) -> bool:
    """Print the summary table. Returns True if all required gates passed."""
    ok = all(not o.blocking_failure for o in outcomes)
    rows = [(o.name, o.status.value, "required" if o.required else "info", o.detail) for o in outcomes]
    try:
        from rich.console import Console
        from rich.table import Table

        table = Table(title="Stučka eval-runner (ai/05 §5.3 gates)")
        for col in ("gate", "status", "kind", "detail"):
            table.add_column(col)
        for name, status, kind, detail in rows:
            table.add_row(name, status, kind, detail)
        Console().print(table)
    except Exception:  # pragma: no cover - plain ASCII fallback
        print("=" * 78)
        print("Stuchka eval-runner (ai/05 §5.3 gates)")
        print("=" * 78)
        for name, status, kind, detail in rows:
            print(f"  {status.upper():9} {kind:9} {name:16} {detail}")
        print("=" * 78)
    print("RESULT:", "PASS - all required eval gates green." if ok else "FAIL - a required eval gate missed.")
    return ok
