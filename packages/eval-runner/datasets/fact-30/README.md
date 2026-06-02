# fact-30 — 事实核验率评测集 (ai/05 §5.5)

**Gate type: HUMAN PROCESS GATE (not decided automatically).**

`0529/prd` §6.2.4 上线门槛: 事实核验率 >= 80%, by 5 内部审阅人 (rotating annotation, >= 1 of 2 reviewers marks 基本正确 per fact).

This is a human-graded gate: M1 诊断引擎 produces fact cards from a case seed, and human reviewers verify
each card on five dimensions (时间正确 / 金额正确 / 当事人正确 / 法律关系正确 / 引用证据正确). It CANNOT be
decided by an automated script. The eval-runner reports it as a `process` gate (like the L0 legal seam),
never silently passing it.

## Contents
- `manifest.json` — the genuine seed: case-seed pointers + expected fact cards (kind + expected value/date +
  evidence refs), per the §5.5 schema. The expected facts are GENUINE (drawn from the rule-engine / deadline
  golden fixtures); they are NOT auto-scored — they are the ground truth a reviewer checks the M1 output against.
- `blind_review_template.csv` — the per-fact reviewer rubric (one row per expected fact, blank verdict columns).

## Building the full 30-case set
Per §5.9 (Q-PRD-12, ~2 person-months): 30 sample cases x >= 10 fact cards each, annotated by 5 internal
reviewers. The seed here is the W1 "标注规范与样例" deliverable (schema + examples); the full set is the
documented human process. Until the human grading is recorded, the automated runner emits
`status=process, decided=false` for fact-30 (it does not assert 80%).
