#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""One-shot assembler for the top-level LICENSE file.

Concatenates the section headers (0..6) around the authentic GPL-3.0-only text
that was fetched into LICENSES/GPL-3.0-only.txt, so the LICENSE embeds the REAL
full license text rather than a paraphrase. Run once to (re)generate LICENSE.
Zero Emoji. The appendix wording is verbatim from spec/compliance/04 3.2 / 3.3.
"""
import os

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), os.pardir, os.pardir))
GPL_PATH = os.path.join(ROOT, "LICENSES", "GPL-3.0-only.txt")
OUT = os.path.join(ROOT, "LICENSE")

with open(GPL_PATH, "r", encoding="utf-8", newline="") as fh:
    gpl_text = fh.read().rstrip("\n")

HEADER = """\
================================================================================
Stučka 项目许可证 (LICENSE)
================================================================================

本文件由六节构成：
  §0  许可证标识 (SPDX: GPL-3.0-only)
  §1  GNU 通用公共许可证第 3 版 (GPL-3.0-only) 完整原文
  §2  附录 A · 关于用途的道德陈述（声明性条款，无法律强制力）
  §3  附录 B · 项目主体登记信息（动态填充）
  §4  附录 C · 律所法律意见书引用（动态填充）
  §5  附录 D · 商标声明（"Stučka" 名称使用边界）
  §6  附录 E · 贡献者许可

具有法律约束力的许可证是 §1 的 GPL-3.0-only 完整原文。
附录 A 至 E 中，仅附录 B、C、E 含与项目治理相关的声明；附录 A、D 为立场陈述与
商标声明，均不构成 GPL-3.0-only §7 意义上的"附加条件"，不限制您的任何法律自由。

本项目其余许可（文档 CC BY-SA 4.0、知识库数据 CC0 1.0）的完整原文见 LICENSES/
目录，遵循 REUSE 规范。


================================================================================
§0  许可证标识
================================================================================

SPDX-License-Identifier: GPL-3.0-only

项目代码（Rust crates + Flutter Dart + 配套脚本）整体以 GPL-3.0-only 授权。
本节标识具有规范效力：当本文件其余内容与 §1 原文存在任何歧义时，以 §1 原文为准。


================================================================================
§1  GNU 通用公共许可证第 3 版 (GPL-3.0-only) 完整原文
================================================================================

"""

APPENDIX_A = """\


================================================================================
§2  附录 A · 关于用途的道德陈述
================================================================================

附录 A · 关于用途的道德陈述（声明性条款，不具有 GPLv3 § 7 意义上的附加条件）

本项目（Stučka）面向中国大陆劳动纠纷处理场景，
默认服务对象为劳动者，特别是低收入、低数字技能的工人群体。

我们承认 GPLv3 的核心精神在于保障下游用户的使用、修改与再分发自由。
本附录不构成对 GPLv3 § 7 意义上的"附加条件"，不限制您的法律自由。

我们仅作为开发者向使用者表达：
- 本项目欢迎用于劳动者维权与法律自助场景；
- 本项目不欢迎被改造用于雇主反劳动者维权场景，
  包括但不限于：识别并打压维权劳动者、伪造解雇证据、
  规避用工责任、生成对劳动者不利的话术；
- 任何与上述非欢迎场景相关的衍生作品，
  本项目方拒绝以"Stučka" 名义为其背书，
  且拒绝接受任何此类衍生作品向上游回流的 Pull Request。

本附录的执行依赖社区共识与道德约束，
不构成对您 GPLv3 自由的任何法律限制。
"""

APPENDIX_B = """\


================================================================================
§3  附录 B · 项目主体登记信息（动态填充）
================================================================================

本附录登记当前承担本项目法律责任的主体信息。字段来源：
legal/subject/current.yaml（CI 守门脚本 tools/release/check_l0_guard.py 据此校验）。

当前主体状态（L0-01 尚未由个人切换到法人主体）：

  主体类型 (type)            : individual（个人开发者）
  主体名称 (name)            : Stučka 维护者（个人）
  法人登记号 (registration_id): 无（个人主体阶段不适用）
  注册法域 (jurisdiction)     : 无（个人主体阶段不适用）
  主体确认 (confirmed)        : false
  投诉申诉邮箱 (contact_email): 见 legal/subject/current.yaml

依据 prd/05-non-functional.md §5.3.5 与 spec/compliance/03 §3：
个人开发者阶段法律责任人格化，法人主体（非营利基金会 / 公司）设立前
不发布公开二进制。法人主体确认后，本附录将回填登记号、注册地与联系方式
（spec/compliance/03 §4.3 动作 C-01）。
"""

APPENDIX_C = """\


================================================================================
§4  附录 C · 律所法律意见书引用（动态填充）
================================================================================

本附录引用律所就本项目备案义务（L0-02）出具的法律意见书元数据。字段来源：
legal/opinions/latest.yaml。意见书原文与扫描件不进开源仓库，仓库仅保留
元数据与 SHA-256 hash（spec/compliance/03 §5.3）。

当前意见书状态：

  意见书编号 (opinion_id) : 尚未出具
  出具日期 (issued_at)    : 尚未出具
  结论 (verdict)          : 尚未出具（none）
  原文 SHA-256 (sha256)   : 尚未出具
  失效日期 (expires_at)   : 尚未出具

依据 prd/05-non-functional.md §5.3.3：法律意见书完成前不公开 GitHub Release
二进制。意见书出具后，本附录将回填编号、出具日期、结论与原文 SHA-256，
README 第二行亦由 tools/release/render_readme_header.py 同步重渲染。
"""

APPENDIX_D = """\


================================================================================
§5  附录 D · 商标声明
================================================================================

附录 D · 商标声明

"Stučka" 是本项目的命名（取自苏俄法学家 P.I. Stučka）。
本项目不对该名称主张商标权。

允许下游：
- 在 fork 中保留 "Stučka" 命名，同时附加自身后缀（如 "Stučka-Fork-X"）；
- 在精神承续的衍生项目中提及 "基于 Stučka"。

不允许下游：
- 在不修改任何代码、纯换名营销的二次发布中使用 "Stučka"；
- 以 "官方 Stučka" 名义发布未由本项目主体维护的版本。
"""

APPENDIX_E = """\


================================================================================
§6  附录 E · 贡献者许可
================================================================================

附录 E · 贡献者许可

本项目不要求贡献者签署 CLA（Contributor License Agreement），亦不要求贡献者
转让版权。

- 版权归属：每位贡献者保留其贡献内容的版权。
- 许可授予：贡献者通过提交贡献，许可本项目方及全体下游接收者按 GPL-3.0-only
  （即 §1 原文规定的条款）使用、修改并再分发其贡献内容。
- 来源证明：贡献者通过在每个 commit 添加 DCO（Developer Certificate of Origin）
  签名行 `Signed-off-by: Name <email>`，证明其有权按上述条款提交该贡献
  （详见 CONTRIBUTING.md §2）。
- 一致性：贡献内容的许可必须与项目整体许可（GPL-3.0-only）兼容；不兼容许可的
  贡献不予合并。

本附录不要求贡献者放弃任何 GPL-3.0-only 赋予的自由，亦不向贡献者施加任何
GPL-3.0-only §7 意义上的附加限制。
"""

content = (
    HEADER
    + gpl_text
    + APPENDIX_A
    + APPENDIX_B
    + APPENDIX_C
    + APPENDIX_D
    + APPENDIX_E
    + "\n"
)

with open(OUT, "w", encoding="utf-8", newline="\n") as fh:
    fh.write(content)

print("LICENSE written:", OUT, "bytes=%d" % len(content.encode("utf-8")))
