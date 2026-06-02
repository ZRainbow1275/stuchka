"""Stučka evaluation harness (ai/05).

The Rust `eval-cli` (backend/crates/eval-cli) is the genuine compute oracle — it links the real
rule-engine / hsd / document / data-model crates and runs the datasets against them. This Python
package owns the §5.3 上线门槛 (thresholds.py), validates the oracle's reports, and decides
pass/fail per gate, exiting non-zero on any required-gate miss (cli.py).
"""

__all__ = ["thresholds", "common", "backend", "cli"]
