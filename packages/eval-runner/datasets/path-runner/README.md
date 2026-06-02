# path-runner — 主用户路径 30-60 分钟评测集 (ai/05 §5.8)

**Status: DEFERRED to R1b** (`0529/prd` §6.2.8 + ai/05 §5.12: 闭群 5 名工人志愿者, R1b 上线门槛).

This is a timed end-to-end usability run (T0 应用启动 -> T7 卷宗包导出) measured against the phase targets in
`timer_schema.json`, recorded per volunteer in `result_template.csv`. It requires the Flutter UI Playwright
hooks and closed-group worker volunteers (R1b). The TypeScript harness shape is in ai/05 §5.8
(`packages/eval-runner/ts/path_runner.ts`). It is **not fabricated** — the automated runner reports
`status=deferred_r1b, decided=false`.
