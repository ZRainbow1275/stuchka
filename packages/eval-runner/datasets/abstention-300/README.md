# abstention-300 — 拒答评测集 (ai/05 §5.6, detail in 03-inv08-three-tier.md §3.7-§3.8)

**Status: DEFERRED to R1b** (ai/05 §5.12 + the R 阶段 line: 拒答/主用户路径 are [R1b 争取], not R1a 必死).

上线门槛 (R1b): 拒答率 <= 10% + 桶命中 >= 90%, over 300 prompts (三档各 100: in-scope / boundary /
out-of-scope). This requires the INV-08 three-tier confidence path-runner interface (run via
`dispatcher.answer`), which is R1b scope. It is **not fabricated here** — building it on real prompts is
tracked as R1b work. The automated runner reports abstention-300 as `status=deferred_r1b, decided=false`;
it never silently passes.
