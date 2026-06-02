# law-200 — 法条准确率评测集 (ai/05 §5.2 + §5.9)

Two layers:

1. **Structural URN validity == 100% — AUTOMATED (REQUIRED).** Run by `eval-cli law` /
   `eval_runner.run_law_200`: every cited statute parses + round-trips through the production D8 parser
   (`data_model::parse_law_ref`). This proves there are no hallucinated or malformed law ids. The seed
   `manifest.json` holds 50 GENUINE URNs taken verbatim from production code (DeadlineKind::law_refs,
   compute/* law_ref constants, the §0.5 parser exemplars, kb SAMPLE_LAWS) and additional real articles of
   those same statutes. Wired into `tools/ci/pre_release.py`.

2. **法条准确率 >= 95% — HUMAN PROCESS GATE.** `0529/prd` §6.2.1: a 200-item bank graded by 2 执业律师 +
   1 法学讲师, blind, cross-annotated (>= 3 disagreements go to arbitration), with root-cause labels on
   failures (~2 person-months, §5.9). This measures whether the dispatcher's ANSWERS cite the correct law,
   which only lawyers can judge. The automated structural gate above is the machine-checkable proxy; the
   full lawyer-graded set is the documented human process and is NOT auto-passed.
