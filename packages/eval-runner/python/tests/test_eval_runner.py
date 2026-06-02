"""Tests for the eval-runner: thresholds match the spec + the report validator rejects bad data.

These run without cargo (they do not invoke the oracle); the genuine gate execution is enforced by
the Rust smoke tests (backend/crates/eval-cli/tests/smoke.rs) and by `python -m eval_runner.cli`.
"""
import pytest

from eval_runner import thresholds
from eval_runner.common import GateReport, Status


def test_spec_thresholds_exact():
    assert thresholds.CALC_50_PASS_RATE_MIN == 1.0
    assert thresholds.DEADLINE_30_PASS_RATE_MIN == 1.0
    assert thresholds.GB45438_COMPLETENESS_MIN == 1.0
    assert thresholds.PII_200_RECALL_MIN == 0.95
    assert thresholds.PII_200_FP_RATE_MAX == 0.05
    assert thresholds.LAW_200_ACCURACY_MIN == 0.95
    assert thresholds.DOC_20_USABLE_RATE_MIN == 0.70
    assert thresholds.FACT_30_VERIFIED_RATE_MIN == 0.80
    assert thresholds.ABSTENTION_REFUSAL_RATE_MAX == 0.10


def test_report_from_dict_roundtrip():
    raw = {
        "gate": "calc",
        "total": 2,
        "passed": 1,
        "failed": 1,
        "pass_rate": 0.5,
        "metrics": {},
        "cases": [
            {"id": "a", "pass": True, "expected": "x", "actual": "x"},
            {"id": "b", "pass": False, "expected": "y", "actual": "z"},
        ],
    }
    r = GateReport.from_dict(raw)
    assert r.total == 2 and r.passed == 1
    assert r.failing_ids() == ["b"]


def test_report_rejects_missing_keys():
    with pytest.raises(Exception):
        GateReport.from_dict({"gate": "calc"})


def test_status_values():
    assert Status.PROCESS.value == "process"
    assert Status.DEFERRED.value == "deferred_r1b"
