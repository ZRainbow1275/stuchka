#!/usr/bin/env python3
"""Deterministically generate the abstention-300 manifest (ai/05 §5.6, ai/03 §3.3 / §3.7-§3.8).

Ground truth is GENUINE, never fabricated. Each case carries a confidence-signal profile; the
expected INV-08 bucket is derived HERE from the ai/03 §3.3 spec boundaries (an independent
restatement), while the Rust oracle (`eval-cli abstention`) drives the PRODUCTION
`compute_confidence -> bucket -> compose` path and the genuine refusal answers. The two agree only
when the production implementation matches the spec:

  - INV-01 rule-coverage dominance fixes the bucket regardless of the other signals:
      exact       -> 0.95            -> High
      approximate -> 0.80            -> High   (returned as a constant, so robust at any level)
      boundary    -> cap 0.70 * pen  -> Mid    (only at level0/1 where level_penalty == 1.0)
      unknown     -> <= 0.49         -> Low    (RULE_ABSTENTION_CAP, robust at any level)
  - The Low bucket is a heuristic FOLLOW-UP, NOT a refusal (PM P5 / INV-08): `compose_low` must
    leave `abstained == false`. Only the genuine refusal answers abstain:
      level4_stale           -> Answer::level4_warning()      (abstained, Low)
      hsd_blocked_no_local   -> Answer::hsd_blocked_no_local() (abstained, Low)

Layout (三档各 100, ai/05 §5.6): 100 in-scope (High) / 100 boundary (Mid) / 100 out-of-scope (Low).
~8% of the out-of-scope tier are genuine hard refusals (level4 / hsd), so the healthy baseline is
refusal_rate = 24/300 = 0.08 (<= 0.10) and bucket_hit = 1.0 (>= 0.90). A regression that breaks the
cap, the boundary, or makes a Low follow-up refuse moves these metrics and fails the gate.

Run: `python generate.py`  ->  writes manifest.json (deterministic; no RNG, no fabricated answers).
"""
import json
import os

# Levels at which a given coverage tag's bucket is ROBUST (independent of the fusion weights).
ALL_LEVELS = ["level0", "level1", "level2", "level3", "level4"]
BOUNDARY_LEVELS = ["level0", "level1"]  # penalty 1.0 keeps boundary in [0.50, 0.70] -> Mid


def _signals(level: str, coverage: str, kb_top: float, idx: int) -> dict:
    """A realistic, deterministic signal profile. The coverage tag dominates the bucket (INV-01);
    the other signals vary for breadth but never change the spec-derived bucket for these tiers."""
    # Deterministic logprob/self-report variation (index-derived, no RNG).
    lp = round(-0.05 - (idx % 7) * 0.03, 4)
    self_rep = round(0.55 + (idx % 5) * 0.08, 4)
    return {
        "model_logprobs": [lp, round(lp - 0.02, 4)],
        "model_self_reported": self_rep,
        "kb_hit_scores": [round(kb_top, 4)],
        "kb_top_score": round(kb_top, 4),
        "rule_outcome": coverage,
        "hsd_route_forced_local": (idx % 4 == 0),
        "level": level,
    }


def build_cases() -> list[dict]:
    cases: list[dict] = []

    # --- Tier 1: 100 in-scope -> High (exact / approximate, robust at every level) ---
    for i in range(100):
        coverage = "exact" if i % 2 == 0 else "approximate"
        level = ALL_LEVELS[i % len(ALL_LEVELS)]
        kb_top = 0.60 + (i % 40) * 0.01  # 0.60 .. 0.99
        cases.append({
            "id": f"abst-inscope-{i:03d}",
            "tier": "in_scope",
            "route": "compose",
            "signals": _signals(level, coverage, kb_top, i),
            "expect": {"bucket": "high", "abstained": False},
        })

    # --- Tier 2: 100 boundary -> Mid (boundary coverage, level0/1 only) ---
    for i in range(100):
        level = BOUNDARY_LEVELS[i % len(BOUNDARY_LEVELS)]
        kb_top = (i % 50) * 0.02  # 0.00 .. 0.98 -> base 0.50 .. 0.70 -> Mid
        cases.append({
            "id": f"abst-boundary-{i:03d}",
            "tier": "boundary",
            "route": "compose",
            "signals": _signals(level, "boundary", kb_top, i),
            "expect": {"bucket": "mid", "abstained": False},
        })

    # --- Tier 3: 100 out-of-scope -> Low ---
    # 76 heuristic follow-ups (unknown coverage; MUST NOT refuse) + 24 genuine refusals.
    for i in range(76):
        level = ALL_LEVELS[i % len(ALL_LEVELS)]
        kb_top = 0.10 + (i % 30) * 0.03  # any value: unknown caps <= 0.49 -> Low
        cases.append({
            "id": f"abst-oos-{i:03d}",
            "tier": "out_of_scope",
            "route": "compose",
            "signals": _signals(level, "unknown", kb_top, i),
            "expect": {"bucket": "low", "abstained": False},
        })
    # 12 Level-4 KB-stale refusals (compensation refused; real Answer::level4_warning).
    for i in range(12):
        cases.append({
            "id": f"abst-refuse-l4-{i:03d}",
            "tier": "out_of_scope",
            "route": "level4_stale",
            "signals": {},
            "expect": {"bucket": "low", "abstained": True},
        })
    # 12 HSD-blocked-no-local refusals (INV-05; real Answer::hsd_blocked_no_local).
    for i in range(12):
        cases.append({
            "id": f"abst-refuse-hsd-{i:03d}",
            "tier": "out_of_scope",
            "route": "hsd_blocked_no_local",
            "signals": {},
            "expect": {"bucket": "low", "abstained": True},
        })

    return cases


def main() -> None:
    cases = build_cases()
    refusals = sum(1 for c in cases if c["expect"]["abstained"])
    manifest = {
        "dataset": "abstention-300",
        "spec": "0529/spec/ai/05-evaluation-set.md §5.6 + ai/03 §3.3 / §3.7-§3.8",
        "threshold": "refusal_rate <= 0.10 AND bucket_hit >= 0.90",
        "ground_truth": (
            "Expected bucket derived from ai/03 §3.3 boundaries (INV-01 rule-coverage dominance + "
            "0.5/0.8 cut points), independent of the crate. The Rust oracle drives the production "
            "compute_confidence->bucket->compose path + the genuine refusal answers "
            "(level4_warning / hsd_blocked_no_local). No answer text is fabricated. "
            "Generated by generate.py (deterministic, no RNG)."
        ),
        "counts": {
            "in_scope_high": 100,
            "boundary_mid": 100,
            "out_of_scope_low": 100,
            "genuine_refusals": refusals,
            "total": len(cases),
        },
        "cases": cases,
    }
    out = os.path.join(os.path.dirname(__file__), "manifest.json")
    with open(out, "w", encoding="utf-8") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=1)
        f.write("\n")
    print(f"wrote {out}: {len(cases)} cases, {refusals} genuine refusals "
          f"(refusal_rate={refusals / len(cases):.4f})")


if __name__ == "__main__":
    main()
