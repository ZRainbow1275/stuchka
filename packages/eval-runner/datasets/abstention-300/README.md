# abstention-300 — 拒答评测集 (ai/05 §5.6, detail in 03-inv08-three-tier.md §3.7-§3.8)

**Status: AUTOMATED + REQUIRED (R1b, WAVE 4).** 上线门槛: 拒答率 <= 10% + 桶命中 >= 90%, over 300
cases (三档各 100: in-scope / boundary / out-of-scope).

## What is genuine here

The dataset (`manifest.json`, produced by `generate.py`, deterministic) carries one
confidence-signal profile per case. The **expected bucket** is derived in the generator from the
ai/03 §3.3 boundaries — an independent restatement of INV-01 rule-coverage dominance + the 0.5/0.8
cut points — NOT from the crate under test.

The Rust oracle (`eval-cli abstention`, links the REAL `ai_dispatcher` crate) drives the PRODUCTION
path for every case:

- `route=compose` → `compute_confidence` → `bucket` → `compose` (the INV-08 three-tier).
- `route=level4_stale` → `Answer::level4_warning()` (genuine refusal: KB stale, compensation off).
- `route=hsd_blocked_no_local` → `Answer::hsd_blocked_no_local()` (genuine refusal: INV-05, no local model).

It reports two metrics; `eval_runner.run_abstention_300` decides them against `thresholds.py`:

- `bucket_hit`   = fraction of cases whose REAL `inv08_bucket` matches the spec-derived expectation.
- `refusal_rate` = fraction of cases whose REAL answer abstains (`abstained == true`).

The expectation and the production path agree only when the implementation matches the spec, so a
broken `RULE_ABSTENTION_CAP`, a moved boundary, or a regression that turns a Low **heuristic
follow-up** into a hard refusal (PM P5 / INV-08 violation) surfaces as a real failure — it can never
silently pass. Healthy baseline: `bucket_hit = 1.0`, `refusal_rate = 24/300 = 0.08`.

## Not covered here (honest seam)

The **human path-runner** (ai/05 §5.6: 5 volunteers running the real desktop app end-to-end, 30–60
min completion window) is a separate process gate under `datasets/path-runner/` and is **never
auto-passed** — it requires real users + the running application. This automated set validates the
INV-08 confidence/abstention machinery; it does not substitute for the human usability path.

Regenerate: `python generate.py` → writes `manifest.json`.
