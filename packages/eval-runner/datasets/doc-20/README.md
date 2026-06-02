# doc-20 — 文书可用率评测集 (ai/05 §5.4)

Two distinct gates over the same 20 generated documents:

1. **GB45438 four-layer completeness == 100% — AUTOMATED (REQUIRED).** Run by `eval-cli doc` /
   `eval_runner.run_doc_20`: every document is produced through the REAL `document::export_dossier`
   (the UNIQUE four-layer injection path, INV-02) and `LayerCompleteness.assert_all_true()` must hold
   on all 20. This is wired into `tools/ci/pre_release.py`.

2. **文书可用率 >= 70% — HUMAN PROCESS GATE.** `0529/prd` §6.2.3: 3 执业律师 sign an NDA and grade the
   documents blind (not told whether AI-generated) on directly-submittable (1-5), 法条嵌入, 事实陈述,
   请求项, 证据列表, 格式排版, GB45438 标识. Usable = ">= 4 of 5 by >= 2 of 3 lawyers". This CANNOT be
   decided automatically; the runner reports it as a `process` gate and never silently passes it.
   `blind_rubrics_template.csv` is the blind-review input form.

`manifest.json` documents the 6 hard templates x 3 scenarios + 2 fallback = 20 documents that the
automated gate generates and checks.
