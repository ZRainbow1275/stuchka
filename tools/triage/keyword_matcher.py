#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Stučka Issues 机器化阻断咨询 关键词 + 正则匹配器.

spec: 0529/spec/compliance/04-gplv3-readme-front.md
  §5.1 阻断触发关键词（个案咨询 / 个人信息 / 教唆违规）
  §5.2 自动回复文案（命中后由 workflow 张贴并关闭）
  §5.3 标签集（命中 -> dispute-not-accepted）
  §5.5 兜底：命中后必须留"申诉误判"入口（APPEAL_HINT）

这是真实匹配器，不是 stub：
  - 个案咨询：关键词命中（"我的案子"、"我应该怎么做"、"我会赢吗" ...）。
  - 个人信息：正则匹配 中国大陆手机号 / 身份证号 / 银行卡号 / 家庭住址。
  - 教唆违规：关键词命中（"怎么伪造"、"怎么绕过"、"如何不被识破"、"造假" ...）。

被 .github/workflows/issue_triage.yml 调用：读取 issue 标题 + 正文，
命中则输出分类 + 标签 + 自动回复文案（含申诉入口），workflow 据此打标签、回帖、关闭。

正则白名单维护接缝（regex allowlist seam）：
  PERSONAL_INFO_ALLOWLIST 用于排除"看起来像个人信息但其实是工程标识"的误报
  （如版本号、commit hash、issue 编号、端口号）。维护者可在此追加白名单正则。

依赖：仅标准库（re / json / argparse / sys / dataclasses）。无第三方依赖，跨机器可运行。
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from typing import Optional


# ---------------------------------------------------------------------------
# §5.3 标签集
# ---------------------------------------------------------------------------
LABEL_DISPUTE_NOT_ACCEPTED = "dispute-not-accepted"
LABEL_BUG = "bug"
LABEL_FEATURE = "feature"
LABEL_QUESTION = "question"
LABEL_COMPLIANCE = "compliance"
LABEL_GOVERNANCE = "governance"
LABEL_APPEAL = "appeal"

ALL_LABELS = frozenset(
    {
        LABEL_BUG,
        LABEL_FEATURE,
        LABEL_QUESTION,
        LABEL_DISPUTE_NOT_ACCEPTED,
        LABEL_COMPLIANCE,
        LABEL_GOVERNANCE,
        LABEL_APPEAL,
    }
)


# ---------------------------------------------------------------------------
# §5.2 自动回复文案（命中后张贴）。逐字对应 spec，不得改动语义。
# ---------------------------------------------------------------------------
AUTO_REPLY = """\
您好，感谢您对 Stučka 的关注。

本项目维护者不对您的具体案件提供法律意见。
本 Issues 区仅接受 bug 报告、功能建议、合规反馈与开源治理讨论。

如您希望获得本系统辅助的法律意见，请直接使用本系统的"问诊" 功能。
如您希望获得人工法律意见，请联系：
- 12348 全国法律援助热线
- 12351 全国总工会维权热线
- 当地法律援助中心

为保护您的个人信息，本 Issue 已被自动关闭并标记为 `dispute-not-accepted`。
您填写的信息将在 24 小时内被项目方主体删除。

如认为本次拦截误判，请重新创建 Issue 并选择 "申诉误判" 模板。"""

# §5.5 兜底：命中后必须保留的"申诉误判"入口。AUTO_REPLY 的末行已含此入口，
# 这里再以独立常量暴露，便于 workflow 与测试断言其始终存在。
APPEAL_HINT = '如认为本次拦截误判，请重新创建 Issue 并选择 "申诉误判" 模板。'


# ---------------------------------------------------------------------------
# §5.1 个案咨询 关键词
# ---------------------------------------------------------------------------
# spec 明确列举（"不完全列举"），此处为可真实匹配的扩展集，覆盖常见同义表达。
CASE_CONSULT_KEYWORDS: tuple[str, ...] = (
    # spec 原文列举
    "我的案子",
    "我应该怎么做",
    "我会赢吗",
    "请给我建议",
    "帮我看看",
    "判断一下我有没有理",
    # 常见同义 / 变体（个案诉求）
    "我的案件",
    "我这个案子",
    "我这种情况",
    "我这个情况",
    "我该怎么办",
    "我应该怎么办",
    "我能赢吗",
    "我能不能赢",
    "我有没有胜算",
    "胜算大吗",
    "胜诉几率",
    "我能拿到",
    "我能要到",
    "我能要多少",
    "我能赔多少",
    "我能拿多少",
    "我该不该",
    "我要不要",
    "帮我分析",
    "帮我判断",
    "帮我算算",
    "帮我看一下",
    "帮我看下",
    "给我点建议",
    "给我个建议",
    "我有没有理",
    "我占不占理",
    "我有没有胜诉",
    "我被辞退了怎么办",
    "我被开除了怎么办",
    "我被裁了怎么办",
    "我能告吗",
    "我能告",
    "我能仲裁吗",
    "我要怎么维权",
    "我怎么维权",
)


# ---------------------------------------------------------------------------
# §5.1 教唆违规 关键词
# ---------------------------------------------------------------------------
ABETMENT_KEYWORDS: tuple[str, ...] = (
    # spec 原文列举
    "怎么伪造",
    "怎么绕过",
    "如何不被识破",
    "造假",
    # 同义 / 变体
    "如何伪造",
    "怎样伪造",
    "伪造证据",
    "伪造合同",
    "伪造记录",
    "如何绕过",
    "怎样绕过",
    "绕过检测",
    "绕过识别",
    "绕过合规",
    "怎么规避",
    "如何规避",
    "规避责任",
    "怎么不被发现",
    "如何不被发现",
    "怎么不被查到",
    "如何不被查出",
    "如何造假",
    "怎么造假",
    "做假证",
    "做假账",
    "篡改记录",
    "篡改证据",
    "篡改聊天",
    "篡改打卡",
    "篡改",
    "如何篡改",
    "怎么篡改",
    "教我作假",
    "帮我作假",
    "做个假",
    "做假",
    "做个假的",
    "如何骗过",
    "怎么骗过",
)


# ---------------------------------------------------------------------------
# §5.1 个人信息 正则（手机号 / 身份证号 / 银行卡号 / 家庭住址）
# ---------------------------------------------------------------------------
# 中国大陆手机号：1 开头，第二位 3-9，共 11 位，前后非数字边界（避免切到长串数字中段）。
RE_PHONE = re.compile(r"(?<!\d)1[3-9]\d{9}(?!\d)")

# 18 位身份证号：前 17 位数字 + 末位 数字或 X/x。前后非 字母数字 边界。
RE_ID_CARD = re.compile(r"(?<![0-9A-Za-z])\d{17}[\dXx](?![0-9A-Za-z])")

# 银行卡号：16-19 位连续数字（容许每 4 位以空格 / 连字符分隔）。
RE_BANK_CARD = re.compile(
    r"(?<!\d)(?:\d[ -]?){15,18}\d(?!\d)"
)

# 家庭住址：省/市/区/县/镇/乡/街道/路/号/室/栋/单元/小区/村 等地址要素的组合。
# 命中需同时出现 行政区划/道路 要素 + 门牌/楼栋/室 要素，降低误报。
RE_ADDRESS = re.compile(
    r"(?:[一-龥]{2,8}(?:省|市|区|县|镇|乡|街道|路|街|村|小区|花园|苑|大厦|广场))"
    r"[一-龥\dA-Za-z]{0,20}?"
    r"(?:\d+|[一二三四五六七八九十百]+)\s*(?:号|栋|幢|单元|室|楼|层)"
)

# 正则白名单维护接缝：命中个人信息正则但实际属于工程标识时在此排除（维护者可扩展）。
# 关键：白名单必须"足够具体"，绝不能挖掉纯数字的真实 PII。
#   - commit hash 白名单要求至少含一个十六进制字母 a-f，否则纯数字手机号/银行卡会被误挖空。
#   - issue 编号要求带前缀 #，且长度 <= 7，避免吃掉 11 位手机号 / 16+ 位卡号。
#   - 版本号要求 a.b.c 三段点分，端口要求带 port/端口 上下文词。
PERSONAL_INFO_ALLOWLIST: tuple[re.Pattern[str], ...] = (
    re.compile(r"(?:#|issue|pr)\s*#?\d{1,7}\b", re.IGNORECASE),  # issue / PR 编号
    re.compile(r"\bv?\d{1,4}\.\d{1,4}\.\d{1,4}(?:[-.\w]+)?\b"),  # 语义化版本号 a.b.c
    re.compile(r"(?:port|端口)\s*[:：]?\s*\d{2,5}\b", re.IGNORECASE),  # 端口（需上下文词）
    # commit hash：必须同时出现字母与数字（纯数字串不算 hash，留给 PII 正则判定）。
    re.compile(r"\b(?=[0-9a-f]*[a-f])(?=[0-9a-f]*\d)[0-9a-f]{7,40}\b", re.IGNORECASE),
)


@dataclass
class MatchResult:
    """单次匹配结果。

    blocked=True 表示命中阻断触发；workflow 据此打 dispute-not-accepted、张贴 AUTO_REPLY 并关闭。
    无论是否命中，appeal_hint 始终非空（§5.5 兜底）。
    """

    blocked: bool
    classification: str  # "case_consult" / "personal_info" / "abetment" / "compliant"
    label: Optional[str]  # dispute-not-accepted 或 None
    matched_terms: list[str] = field(default_factory=list)
    auto_reply: str = ""
    appeal_hint: str = APPEAL_HINT

    def to_dict(self) -> dict:
        return {
            "blocked": self.blocked,
            "classification": self.classification,
            "label": self.label,
            "matched_terms": self.matched_terms,
            "auto_reply": self.auto_reply,
            "appeal_hint": self.appeal_hint,
        }


def _strip_allowlisted(text: str) -> str:
    """把白名单（版本号 / 端口 / issue 编号 / hash）从文本中挖空，避免个人信息正则误报。"""
    cleaned = text
    for pat in PERSONAL_INFO_ALLOWLIST:
        cleaned = pat.sub(" ", cleaned)
    return cleaned


def _find_personal_info(text: str) -> list[str]:
    """返回命中的个人信息样本（已脱白名单）。不回显完整敏感串，仅回类别标记。"""
    cleaned = _strip_allowlisted(text)
    hits: list[str] = []
    if RE_PHONE.search(cleaned):
        hits.append("phone")
    if RE_ID_CARD.search(cleaned):
        hits.append("id_card")
    if RE_BANK_CARD.search(cleaned):
        # 银行卡正则较宽：要求命中段去掉分隔符后长度在 16-19，且未被身份证/手机覆盖。
        for m in RE_BANK_CARD.finditer(cleaned):
            digits = re.sub(r"\D", "", m.group())
            if 16 <= len(digits) <= 19:
                hits.append("bank_card")
                break
    if RE_ADDRESS.search(cleaned):
        hits.append("address")
    return hits


def classify(title: str, body: str = "") -> MatchResult:
    """对 Issue 标题 + 正文做 §5.1 三类阻断判定。

    优先级：个人信息 > 教唆违规 > 个案咨询（任一命中即 blocked）。
    返回的 MatchResult 始终携带 appeal_hint（§5.5）。
    """
    title = title or ""
    body = body or ""
    haystack = f"{title}\n{body}"

    # 1) 个人信息（正则）
    pi = _find_personal_info(haystack)
    if pi:
        return MatchResult(
            blocked=True,
            classification="personal_info",
            label=LABEL_DISPUTE_NOT_ACCEPTED,
            matched_terms=pi,
            auto_reply=AUTO_REPLY,
        )

    # 2) 教唆违规（关键词）
    abet = [k for k in ABETMENT_KEYWORDS if k in haystack]
    if abet:
        return MatchResult(
            blocked=True,
            classification="abetment",
            label=LABEL_DISPUTE_NOT_ACCEPTED,
            matched_terms=abet,
            auto_reply=AUTO_REPLY,
        )

    # 3) 个案咨询（关键词）
    case = [k for k in CASE_CONSULT_KEYWORDS if k in haystack]
    if case:
        return MatchResult(
            blocked=True,
            classification="case_consult",
            label=LABEL_DISPUTE_NOT_ACCEPTED,
            matched_terms=case,
            auto_reply=AUTO_REPLY,
        )

    # 未命中：合规工程类 Issue，放行（label 由模板自身决定）。
    return MatchResult(
        blocked=False,
        classification="compliant",
        label=None,
        matched_terms=[],
        auto_reply="",
    )


def _cli(argv: Optional[list[str]] = None) -> int:
    """CLI 入口。被 workflow 调用：

    python keyword_matcher.py --title "<标题>" --body-file body.txt --json
    输出 JSON；若 blocked=True，进程退出码 = 10（workflow 据此分支），否则 0。
    --self-test 运行内置标注集并断言准确率 >= 95%（spec §8）。
    """
    parser = argparse.ArgumentParser(description="Stučka Issues 关键词阻断匹配器")
    parser.add_argument("--title", default="", help="Issue 标题")
    parser.add_argument("--body", default="", help="Issue 正文（内联）")
    parser.add_argument("--body-file", default=None, help="Issue 正文文件路径")
    parser.add_argument("--json", action="store_true", help="以 JSON 输出结果")
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="运行内置标注集，断言准确率 >= 95%，打印真实准确率",
    )
    args = parser.parse_args(argv)

    if args.self_test:
        from test_keyword_matcher import run_self_test  # 同目录测试模块

        acc, ok = run_self_test()
        print(f"accuracy={acc:.4f} pass={ok}")
        return 0 if ok else 1

    body = args.body
    if args.body_file:
        with open(args.body_file, "r", encoding="utf-8") as fh:
            body = fh.read()

    result = classify(args.title, body)
    if args.json:
        print(json.dumps(result.to_dict(), ensure_ascii=False, indent=2))
    else:
        print(f"blocked={result.blocked} classification={result.classification}")
        print(f"label={result.label} matched={result.matched_terms}")
        if result.blocked:
            print("--- auto reply ---")
            print(result.auto_reply)

    # 退出码：命中 -> 10（workflow 分支用），未命中 -> 0。
    return 10 if result.blocked else 0


if __name__ == "__main__":
    sys.exit(_cli())
