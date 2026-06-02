# Stučka eval-runner (ai/05)

The evaluation harness for the §5.3 上线门槛. Architecture:

- **`backend/crates/eval-cli`** (Rust, bin `stuchka-eval`) is the genuine compute **oracle**. It links the
  REAL production crates — `rule-engine` (M9 calc + M5 deadline), `hsd` (high-sensitivity detector),
  `document` (GB45438 four-layer export), `data-model` (D8 LawRef parser) — and runs each dataset against
  them, emitting a per-case JSON report. It performs NO threshold judgement, so a mis-derived expected
  value surfaces as a real failure and can never silently pass. (The spec's `maturin` Rust↔Python bridge
  is realised here as a subprocess + JSON contract — a documented, honest choice, not an FFI mock.)
- **`packages/eval-runner`** (this package, Python) owns the §5.3 thresholds (`thresholds.py`), validates
  the oracle's reports (pydantic when present, stdlib otherwise), decides pass/fail per gate, and exits
  non-zero on any required-gate miss (`cli.py`). Wired into `tools/ci/pre_release.py`.

## Datasets (`datasets/`)
| set | gate type | threshold | ground truth |
|-----|-----------|-----------|--------------|
| calc-50 | automated, required | pass_rate == 1.0 | §47/§87/§82/§44/§85/工伤/个税 law formulas; mirrors rule-engine golden + cross-province non-capped permutations |
| deadline-30 | automated, required | pass_rate == 1.0 | calendar math over real windows (1y / 2y / 15d / 60d); mirrors deadline golden |
| pii-200 | automated, required | recall>=0.95, fp<=0.05 | real-format valid tokens (regex phones, ISO-7064 id cards, Luhn bank cards, keyword paths) vs benign/near-miss |
| doc-20 | automated (GB45438) + human (usability) | completeness==1.0 / usable>=0.70 | real `export_dossier` four-layer completeness; usability is a 3-lawyer blind process gate |
| law-200 | automated (structural) + human (accuracy) | structural==1.0 / accuracy>=0.95 | real PRC statute URNs from production code; accuracy is a lawyer-blind process gate |
| fact-30 | human process | verified>=0.80 | 5-reviewer fact-card verification |
| abstention-300 | deferred R1b | refusal<=0.10 | §5.12 |
| path-runner | deferred R1b | 30-60 min | §5.8 |

## Run
```bash
# all gates (required automated gates block; process/deferred gates report only):
python -m eval_runner.cli            # from packages/eval-runner/python  (or with it on PYTHONPATH)
# optional strict validation + pretty table:
pip install -e .[full]
# unit tests:
pytest python/tests
```

Required automated gates run the real backend; install the Rust toolchain (cargo) to execute them.
