# 贡献者准则 · Stučka

感谢你考虑为 Stučka 做出贡献。本项目是一个本地优先的劳动纠纷处理 AI 工作台，默认服务对象为中国大陆的劳动者，特别是低收入、低数字技能的工人群体。

在提交任何 Issue 或 Pull Request 之前，请完整阅读本文。本文的全部条款与项目许可（顶层 `LICENSE`，SPDX `GPL-3.0-only`）及其附录 A（道德陈述）、附录 E（贡献者许可）一致并互为补充。

---

## 1. 立场声明

本项目重申 INV-09：**反 HR 立场是道德姿态，不是技术防御**。

- 本项目默认偏向工人视角（system prompt、问诊树、文书模板）。这一默认值是开发者向使用者表达的道德姿态，任何人都可以依据 GPLv3 fork 并改变它，项目方不做、也无法做技术层面的强制防御。
- 正因如此，我们不接受“我打算做 HR 版 / 雇主版分支”类提案进入本仓库主线。你有 fork 的自由（这正是 README 第一行所声明的事实），但本上游不为该方向提供工程接口、不予合并、不以 “Stučka” 名义背书（见附录 A、附录 D）。

## 2. DCO（Developer Certificate of Origin）

本项目使用 DCO 而非 CLA。

- 每一个 commit 都必须包含签名行：`Signed-off-by: Your Name <your.email@example.com>`。使用 `git commit -s` 自动添加。
- 签名即表示你声明你的贡献符合 [Developer Certificate of Origin 1.1](https://developercertificate.org/)：你有权按本项目许可（GPL-3.0-only）提交该贡献。
- 项目方在合并前使用 `dco-bot`（或等价 CI 检查）校验每个 commit 的 `Signed-off-by` 行；缺失签名的 commit 不予合并。

## 3. 版权归属（不要求 CLA）

- 贡献者**保留**其贡献内容的版权。
- 贡献者通过提交贡献，**许可项目方及全体下游接收者按 GPL-3.0-only 重分发**其贡献内容（见 `LICENSE` 附录 E）。
- 本项目**不要求 CLA**，**不要求版权转让**。

## 4. 提交规范（Conventional Commits）

- 采用 [Conventional Commits](https://www.conventionalcommits.org/)，**允许中文正文**。
- 格式：`type(scope): subject`，例如 `feat(ai-dispatcher): 增加反 HR 默认段开关`。
- 常用 type：`feat` / `fix` / `docs` / `refactor` / `test` / `chore` / `ci`。
- `scope` 与 `0529/spec` 族对齐（如 `ai-dispatcher`、`document`、`hsd`、`audit`、`compliance`、`frontend`、`editor`）。

## 5. 代码审查

合并任何 PR 之前必须同时满足：

- 至少 **1 名 maintainer** 审查通过；
- **CI 全绿**（含 lint、单元 / 集成测试、Emoji 扫描门禁、`tools/qa/check_readme_first_line.py`）；
- 相关 **INV 验收脚本通过**（例如 GB 45438 四层标识完整率 100%、计算引擎 50 题 100% 正确）。

## 6. 法律合规

涉及以下不变量（INV）的改动，**必须额外经过合规族 maintainer 审查**：

- **INV-02**：GB 45438-2025 四层标识（首页 / 页眉、文件元数据、段落级水印、卷宗 JSON 标注）。
- **INV-05**：数据出境守卫（境外云模型默认禁用 + 出境警示 + 单独同意）。
- **INV-10**：高风险决策强制嵌入完整免责声明 + 冷静期。

任何削弱、绕过或默认关闭上述合规机制的改动，未经合规族 maintainer 审查不得合并。

## 7. 行为准则

- 本项目采用 [Contributor Covenant 2.1](https://www.contributor-covenant.org/version/2/1/code_of_conduct/) 作为社区行为准则。
- 在此基础上，项目方对**反劳动者言论零容忍**：在 Issues / Discussions / PR 评论中发表打压、歧视或教唆侵害劳动者权益的言论，将被移除内容并视情节限制参与。
- 行为准则的执行不区分贡献者身份与贡献量。

## 8. 拒绝接受的 PR

以下类别的 PR 一律 **close（不予合并）**：

- “HR 模式 / 雇主版”功能或开关；
- “绕过 GB 45438 标识”（移除或削弱四层标识）；
- “加密免责文案绕过”（隐藏、加密或默认关闭 INV-10 免责声明 / 冷静期）；
- “出境守卫旁路”（绕过境外云模型默认禁用与出境警示）。

依据 GPLv3，你完全有权在自己的 fork 中做上述任何修改；本条仅约束**向本上游回流**的 PR。

## 9. 知识库贡献元数据

知识库条目（法条、规则、模板）的贡献，**必须附带**以下元数据，否则不予合并：

- **法源原文链接**：可公开访问的官方 / 权威出处 URL。
- **整理者**：条目的整理 / 录入者署名。
- **最后核查日期**：最近一次人工核验该条目仍然有效的日期（`YYYY-MM-DD`）。

此元数据与 **INV-04 案件冻结**协同：案件创建时冻结当时知识库 hash，后续法规更新只生成“影响提示”，不强制改动已冻结案件；准确的核查日期是“影响提示”可信的前提。

---

## 社区治理补充段

我们保留拒绝以下贡献者参与本项目的权利：
- 公开支持反劳动者立场，且其贡献内容指向规避本项目反 HR 默认 prompt 的；
- 在 Issues / Discussions 公开诱导用户违规使用本项目（如教唆伪造证据）的；
- 多次提交“批量代用户问”类 PR / Issue（违反 §5 Issues 机器化阻断咨询）。

拒绝决定由 maintainer 多数表决，并记录在 `governance/decisions/`。
