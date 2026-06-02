本许可证（GPLv3）允许任何人 fork、修改、再分发本项目，
项目方对反向用途（如雇主版分支）无法做技术防御。
反 HR 立场是道德姿态，请用户与开发者基于共同立场使用本项目。

<!-- RENDERED_LEGAL_STATUS_LINE:BEGIN -->
本项目当前由个人维护，律所意见书尚未出具，公开二进制发布暂停。
<!-- RENDERED_LEGAL_STATUS_LINE:END -->

项目主体：当前由个人开发者维护（法律责任人格化，法人主体设立前不发布公开二进制；详见 LICENSE 附录 B）。
投诉与申诉邮箱：stuchka-legal@protonmail.com （亦可通过 GitHub Issues 的“法律投诉 / 申诉误判”模板提交）。
紧急下架预案：如本仓库被下架，备用镜像与最新状态请联系 stuchka-legal@protonmail.com 获取。

> 上面三行（强制承认行、动态法律状态行、主体与申诉行）是本项目对 INV-09 “反 HR 是道德姿态，不是技术防御” 的对外工程契约。第一行不允许任何形式的折叠、修饰或语言切换隐藏，由 CI 门禁 `tools/qa/check_readme_first_line.py` 逐字节校验；第二行由 `tools/release/render_readme_header.py` 依据 `legal/opinions/latest.yaml` 在每次构建时重渲染，保证文档与真实法律状态一致。

---

# Stučka

Stučka 是一个本地优先（local-first）的劳动纠纷处理 AI 工作台，面向中国大陆劳动纠纷场景，默认服务对象为劳动者，特别是低收入、低数字技能的工人群体（包括但不限于农民工、外卖骑手、网约车司机、平台灵活就业者、家政、建筑工、劳务派遣）。

项目命名取自苏俄法学家 P.I. Stučka。

## 这是什么

- 本地优先：核心数据与推理调度在用户本机进行，不托管模型推理，不向公众提供 SaaS。模型权重不打包，由用户自带（境内云模型默认推荐，境外云模型需主动开启并经出境警示）。
- 反 HR 默认立场：官方默认 system prompt、问诊树与文书模板默认偏向工人视角。该立场是道德姿态，不是技术防御——任何人都可以 fork 并改变它，这正是本仓库首行所声明的事实。
- 合规内建：生成文书带 GB 45438-2025 四层标识；高敏信息本地检测；高风险决策强制嵌入完整免责与冷静期。

## 许可

| 范围 | 许可 |
|------|------|
| 项目代码（Rust crates + Flutter Dart + 配套脚本） | GPL-3.0-only |
| 项目文档（含 spec / prd / decision-pack） | CC BY-SA 4.0 |
| 知识库数据（法条整理、规则枚举） | CC0 1.0（公有领域） |
| 模型权重 | 不打包，用户自带 |

完整许可文本见顶层 `LICENSE`（含 GPL-3.0-only 完整原文 + 附录 A 道德条款 / B 主体登记 / C 律所意见引用 / D 商标声明 / E 贡献者许可），以及 REUSE 规范布局的 `LICENSES/` 目录：

```
LICENSES/
├── GPL-3.0-only.txt
├── CC-BY-SA-4.0.txt
└── CC0-1.0.txt
```

SPDX 标识：`GPL-3.0-only`。

## 项目结构

```
Stučka/
├── backend/      Rust core（多 crate workspace；本机子进程，经 127.0.0.1:<随机端口> + Bearer token 通信）
├── frontend/     Flutter Desktop（Windows 桌面端为 R1 必交平台）
├── editor/       Tiptap / ProseMirror 文书编辑器（WebView2 内嵌，WebMessageChannel 通信）
├── LICENSE       顶层许可证（GPL-3.0-only 完整原文 + 附录 A–E）
├── LICENSES/     REUSE 布局多许可声明（GPL-3.0-only / CC-BY-SA-4.0 / CC0-1.0）
├── CONTRIBUTING.md 贡献者准则（含 DCO、立场声明、行为准则、拒收 PR 类型）
├── legal/        法律元数据（主体登记 / 律所意见，仅元数据与 hash，不含原文扫描件）
├── tools/        发布与质量门禁脚本（render_readme_header.py / check_readme_first_line.py 等）
└── .github/      Issues 模板、机器化阻断咨询 workflow、关键词匹配
```

技术栈要点：Rust core 子进程 + Flutter Desktop 主进程（D1：经本地回环 HTTP + Bearer token 通信，已废弃 Tauri / FFI / Pigeon / MethodChannel）；文书编辑器基于 Tiptap，运行于 WebView2（Windows）并以 WebMessageChannel 通信；同步走 Yjs CRDT（与 HTTP 同端口的 WebSocket）。

## 构建

> 平台范围（prd §5.8.1）：R1 仅交付 Windows 桌面端。macOS / Linux 桌面与 iOS 推迟到 R2 / R1b。Web / PWA 永不交付。

前置依赖：

- Rust 稳定版工具链（见 `backend/rust-toolchain.toml`）。
- Flutter SDK（Desktop 支持已启用：`flutter config --enable-windows-desktop`）。
- Node.js（用于构建 `editor/` 的 Tiptap 产物）。

构建步骤（Windows）：

```
# 1) 构建 Rust core
cd backend
cargo build --release

# 2) 构建文书编辑器产物
cd ../editor
npm ci
npm run build

# 3) 构建 Flutter 桌面端
cd ../frontend
flutter pub get
flutter build windows
```

### 西里尔路径构建提示（构建坊说明）

本项目根目录名 `Stučka` 含西里尔 / 带变音符的字符（`č`）。在 Windows 上，部分构建工具（CMake、个别 Rust build script、Node 原生模块）对非 ASCII 路径或非 UTF-8 代码页支持不稳定，可能在含 `č` 的绝对路径下报错。请采用以下任一规避方式：

- 推荐：先将工作区代码页切换为 UTF-8（`chcp 65001`），再执行上述构建命令。
- 或：将仓库 clone / 检出到纯 ASCII 路径（例如 `C:\src\stuchka`）后再构建；产物与位于 `Stučka` 路径时完全一致。
- 或：使用本项目提供的“西里尔路径构建坊”脚本，在临时 ASCII 工作目录中执行构建，再把产物拷回 `Stučka` 目录（不改变任何源代码，仅隔离构建路径）。

仓库内文件（源码、文档、许可）始终以 UTF-8 + LF 存储，编译时按 `include_str!` 等编译期嵌入方式打包关键文本（如 system prompt），不依赖运行期路径，因此跨版本 / 跨配置 / 跨机器均可构建。

## 质量门禁

```
# README 第一行强制文案逐字节校验（R1 CI 门禁，spec/compliance/04 §8）
python tools/qa/check_readme_first_line.py

# 依据 legal/opinions/latest.yaml 重渲染 README 法律状态行（幂等）
python tools/release/render_readme_header.py
# 仅检查是否需要重渲染（CI 用，不写文件）：
python tools/release/render_readme_header.py --check
```

## 贡献

请先阅读 [`CONTRIBUTING.md`](CONTRIBUTING.md)。要点：每个 commit 必须带 DCO `Signed-off-by` 行；本项目不接受 CLA、不要求版权转让；不接受“HR 模式”“绕过 GB 45438 标识”“加密免责文案绕过”“出境守卫旁路”类 PR。

## 免责

本项目的任何输出不构成法律意见，高风险决策必须经用户审阅，并在必要时联系执业律师或法律援助。本项目列出的任何热线（如 12348 全国法律援助热线、12351 全国总工会维权热线、12320）仅为公开信息，与本项目无任何合作关系。
