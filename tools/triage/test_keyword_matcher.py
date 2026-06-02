#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""keyword_matcher 的真实标注集对照测试.

spec 04-gplv3-readme-front.md §8 验收门槛：
  "100 条触发关键词 + 100 条合规标题做对照测试，自动分类准确率 >= 95%。"

本文件构造 >=100 条"应被拦截"(trigger-positive) + >=100 条"应放行"(compliant) 的真实标注样本，
逐条跑 classify()，统计真实准确率并断言 >= 0.95。

运行方式：
  python -m pytest tools/triage/test_keyword_matcher.py
  或
  python tools/triage/keyword_matcher.py --self-test
  或
  python tools/triage/test_keyword_matcher.py   （直接执行，打印准确率 + 退出码）
"""
from __future__ import annotations

import os
import sys

# 允许直接 `python test_keyword_matcher.py` 时找到同目录的 keyword_matcher。
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from keyword_matcher import classify  # noqa: E402


# ---------------------------------------------------------------------------
# trigger-positive：应被拦截（blocked=True）。每条 (标题, 正文)。
# 覆盖 §5.1 三类：个案咨询 / 个人信息（手机·身份证·银行卡·住址）/ 教唆违规。
# ---------------------------------------------------------------------------
TRIGGER_POSITIVE: list[tuple[str, str]] = [
    # --- 个案咨询 (case_consult) ---
    ("我的案子能赢吗", ""),
    ("帮我看看我这种情况怎么办", ""),
    ("我应该怎么做才能拿到赔偿", ""),
    ("请给我建议，我被辞退了", ""),
    ("判断一下我有没有理", ""),
    ("我会赢吗这个仲裁", ""),
    ("我这个案子有没有胜算", ""),
    ("我能要到多少赔偿金", ""),
    ("帮我分析一下我的劳动合同", ""),
    ("我该不该去仲裁", ""),
    ("我被开除了怎么办", ""),
    ("我能拿到双倍工资吗", ""),
    ("我的案件能不能立案", ""),
    ("帮我算算我能赔多少", ""),
    ("我这种情况能要赔偿吗", ""),
    ("我占不占理，公司没签合同", ""),
    ("帮我看一下我的胜诉几率", ""),
    ("我要怎么维权", ""),
    ("我能告公司吗", ""),
    ("给我点建议吧我很急", ""),
    ("我能不能赢这场官司", ""),
    ("我应该怎么办，工资被拖欠", ""),
    ("帮我看下我的赔偿能拿多少", ""),
    ("我能仲裁吗现在", ""),
    ("我这个情况胜算大吗", ""),
    ("我被裁了怎么办求助", ""),
    ("帮我判断公司违法没有", ""),
    ("我有没有胜诉的可能", ""),
    ("我该不该签这个协议", ""),
    ("我要不要接受公司的方案", ""),
    ("我的案子怎么处理比较好", "在线等"),
    ("公司不给加班费我应该怎么做", ""),
    ("我能要到经济补偿吗", "工作了三年"),
    ("帮我看看这个仲裁结果", ""),
    ("我会赢吗大家帮我看看", ""),
    ("我这种情况怎么维权", ""),
    ("请给我建议，到底该不该忍", ""),
    ("我能拿多少钱赔偿", ""),
    ("帮我分析胜诉几率有多大", ""),
    ("我应该怎么办，HR 让我自己离职", ""),
    # --- 个人信息: 手机号 (personal_info / phone) ---
    ("联系我 13800138000", ""),
    ("我的电话是13912345678请回复", ""),
    ("加我微信同号 13700137000", ""),
    ("有结果打我电话 15012345678", ""),
    ("急，电话13688889999在线等", ""),
    ("", "我的手机号 18811112222 麻烦联系"),
    ("能否私聊我 17712345678", ""),
    ("我手机 13599998888 谢谢", ""),
    ("回电 13266667777", ""),
    ("方便加我吗 13455556666", ""),
    # --- 个人信息: 身份证号 (personal_info / id_card) ---
    ("我的身份证 11010119900307391X", ""),
    ("身份证号110105199001011234求帮忙", ""),
    ("", "证件号 440308199512120011 已上传"),
    ("身份证310115198803034567可以核对", ""),
    ("我是 11022219950101001x 求助", ""),
    # --- 个人信息: 银行卡号 (personal_info / bank_card) ---
    ("工资打到 6222020200112233445", ""),
    ("我的银行卡 6217001234567890123", ""),
    ("", "卡号 6228 4801 0000 0000 123 收款"),
    ("退款到6212260200000000123这张卡", ""),
    ("我的卡号是 6225880137624861 帮我看下", ""),
    # --- 个人信息: 家庭住址 (personal_info / address) ---
    ("我家住北京市朝阳区幸福路18号3单元502室", ""),
    ("", "地址：上海市浦东新区世纪大道100号20楼"),
    ("公司在广东省深圳市南山区科技园路9号楼", ""),
    ("我住在杭州市西湖区文一路888号5幢101室", ""),
    ("送达地址成都市武侯区人民南路四段12号6栋", ""),
    # --- 教唆违规 (abetment) ---
    ("怎么伪造考勤记录", ""),
    ("如何伪造劳动合同", ""),
    ("怎么绕过公司的打卡系统", ""),
    ("如何不被识破我请的假", ""),
    ("教我造假病假条", ""),
    ("怎么伪造加班证据", ""),
    ("如何绕过合规检测", ""),
    ("怎样篡改聊天记录当证据", ""),
    ("帮我做个假的工资流水", ""),
    ("怎么造假社保记录", ""),
    ("如何篡改邮件时间", ""),
    ("怎么不被发现我改了合同", ""),
    ("做假证应该怎么做", ""),
    ("如何骗过仲裁员", ""),
    ("怎么规避竞业限制不被查到", ""),
    ("伪造证据会被发现吗怎么避免", ""),
    ("怎么绕过出境守卫导出", ""),
    ("如何不被查出我伪造的记录", ""),
    ("帮我作假一份离职证明", ""),
    ("怎么伪造微信聊天截图", ""),
    # --- 混合 / 较隐蔽的个案+敏感 ---
    ("我的案子，联系13800001111", ""),
    ("帮我看看，我身份证11010119850505123X", ""),
    ("我会赢吗？地址北京市海淀区中关村大街1号8层", ""),
    ("我应该怎么做，卡号6222021234567890123", ""),
    ("我被辞退了，电话15999998888帮我分析", ""),
    ("我这种情况怎么办 怎么伪造点证据", ""),
    ("请给我建议 我住在天津市和平区南京路100号5门302", ""),
    ("我能赢吗，怎么绕过公司监控", ""),
    ("帮我判断 我的身份证号是 44030819900101551X", ""),
    ("我的案件 联系方式 13511112222", ""),
    ("帮我看看，工资卡 6217856100001234567", ""),
    ("我应该怎么办，如何不被识破", ""),
    ("我会赢吗 我家住深圳市福田区深南大道2008号15楼", ""),
    ("我能要多少 电话13888887777", ""),
    ("我这个案子 身份证 110101200001011239", ""),
    ("帮我算算 卡号 6228480000001234567", ""),
    ("我该不该仲裁 怎么伪造劳动关系证明", ""),
    ("我占理吗 地址济南市历下区泉城路1号9栋", ""),
    ("我能告吗 我手机17600006666", ""),
    ("我有没有胜算 如何篡改打卡数据", ""),
]


# ---------------------------------------------------------------------------
# compliant：应放行（blocked=False）。真实的工程 / 合规 / 治理类标题。
# 含若干"易误报"对照：版本号、端口、commit hash、issue 编号、纯技术数字串。
# ---------------------------------------------------------------------------
COMPLIANT: list[tuple[str, str]] = [
    ("构建失败：cargo build 报 linker error", ""),
    ("Windows 桌面端启动时白屏", ""),
    ("自编译模式下 SiliconFlow 网关连接超时", ""),
    ("文档导航链接 404", ""),
    ("建议增加 macOS arm64 预编译包", ""),
    ("问诊树的返回按钮无响应", ""),
    ("规则引擎在闰年场景下时效计算偏差一天", ""),
    ("knowledge base 增量更新失败提示不清晰", ""),
    ("希望支持深色模式", ""),
    ("CONTRIBUTING.md 的 DCO 签名示例有误", ""),
    ("README 第一行强承认文案排版建议", ""),
    ("GB 45438 四层标识在 PDF 导出时缺失第四层", ""),
    ("出境守卫在离线模式下误报", ""),
    ("HSD 高敏感检测把普通法条引用当成个人信息", ""),
    ("INV-08 三档置信度的中档阈值是否可配置", ""),
    ("LocalQwen 模型完整性校验 SHA-256 不匹配", ""),
    ("文书模板渲染时中文字体缺失", ""),
    ("API /llm 路由返回 500", ""),
    ("如何在 Linux 上从源码编译", ""),
    ("部署文档缺少反向代理配置示例", ""),
    ("合规反馈：免责文案在第 3 步未弹出", ""),
    ("治理讨论：maintainer 评审流程是否应公开", ""),
    ("申诉误判流程的入口在哪里", ""),
    ("Cargo.toml 中 reqwest 版本锁建议升级到 0.12.x", ""),
    ("升级到 v1.2.3 后配置文件不兼容", ""),
    ("服务监听 port 8317 时报地址被占用", ""),
    ("commit a04e54f 引入的回归", ""),
    ("Issue #123 的修复未合并到 main", ""),
    ("端口 2080 的代理在 Docker 内不可达", ""),
    ("十六进制颜色 #1f1f1f 在主题里显示异常", ""),
    ("性能：KB 混合检索在 10000 条目时延迟过高", ""),
    ("文档：补充 INV-09 反 HR 立场的工程映射说明", ""),
    ("ai-dispatcher 的 system prompt 能否 fork 修改", ""),
    ("三阶段 A 到 F 流水线的文档不清晰", ""),
    ("LICENSE 附录 A 道德条款的英文翻译建议", ""),
    ("CC0 知识库数据的引用格式问题", ""),
    ("feature: 支持导出为 Markdown", ""),
    ("bug: 时效倒计时在跨时区时错误", ""),
    ("question: 如何配置 NO_PROXY", ""),
    ("compliance: 备案豁免假设的法律意见书链接失效", ""),
    ("governance: 行为准则引用的 Contributor Covenant 版本", ""),
    ("贡献指南：scope 命名规范不清楚", ""),
    ("Flutter 桌面端窗口缩放后布局错乱", ""),
    ("Rust 核心 crate 的依赖图能否提供 cargo tree 输出", ""),
    ("M5 时效计算模块的单元测试覆盖率", ""),
    ("M7 四层降级 FSM 的状态转移表文档", ""),
    ("数据模型 SourceTag 的枚举值含义", ""),
    ("crypto crate 的密钥派生算法说明", ""),
    ("sync 模块在断网恢复后重复同步", ""),
    ("audit 日志的时间戳精度", ""),
    ("hsd crate 的正则白名单如何扩展", ""),
    ("kb crate 的向量索引构建很慢", ""),
    ("document crate 的水印覆盖范围", ""),
    ("rule-engine 的省份代码 44 表示什么", ""),
    ("onboarding 向导无法选择两个不同的云厂商", ""),
    ("confirm 协议的跨境签名校验失败", ""),
    ("observability 降级事件没有上报", ""),
    ("local_qwen 推理在 R1 是否已实装", ""),
    ("provider switch 在主备都挂时的行为", ""),
    ("routing 决策对 force_local 的处理", ""),
    ("levels 模块 60 秒 tick 是否可调", ""),
    ("inv08_template 的模板能否本地覆盖", ""),
    ("inv08_compose 输出的 key_facts 顺序", ""),
    ("confidence 融合公式的权重来源", ""),
    ("answer 结构体新增字段的兼容性", ""),
    ("wiring 适配器与真实 crate 的对接文档", ""),
    ("error 枚举 E_LLM_PROVIDER_DOWN 的触发条件", ""),
    ("config 解析 secrets.toml 报错信息不友好", ""),
    ("stage 流水线在 Level3 时跳过 AI 是否符合预期", ""),
    ("希望增加批量导入法条的 CLI 工具", ""),
    ("文档站点的搜索框无效", ""),
    ("CI 在 Windows runner 上超时", ""),
    ("clippy 警告 -D warnings 导致构建失败的清单", ""),
    ("pytest 在 tools/triage 下找不到模块", ""),
    ("GitHub Actions workflow 的权限配置", ""),
    ("issue 模板的必填校验不生效", ""),
    ("config.yml 的 blank_issues_enabled 设置无效", ""),
    ("keyword_matcher 的正则白名单维护文档", ""),
    ("建议把自动回复文案抽成可配置文件", ""),
    ("triage workflow 误关闭了我的 bug 报告", ""),
    ("如何为知识库条目补充法源链接", ""),
    ("INV-04 案件冻结与知识库版本的关系", ""),
    ("INV-01 物理隔离的依赖图断言怎么跑", ""),
    ("INV-05 强制本地的触发条件文档", ""),
    ("INV-10 群体案件免责模板归属哪个文档", ""),
    ("D6 决策 system prompt 归属变更记录", ""),
    ("master-index 的 LawRef 格式说明", ""),
    ("0529 spec 与 0512 决策包的差异", ""),
    ("editor 模块的快捷键冲突", ""),
    ("frontend 路由跳转闪烁", ""),
    ("backend 启动脚本在 macOS 上权限不足", ""),
    ("如何贡献一个新的文书模板", ""),
    ("文档错别字：经济补偿写成经济补长", ""),
    ("建议支持繁体中文界面", ""),
    ("性能优化：减少冷启动时间", ""),
    ("可访问性：屏幕阅读器无法读取按钮标签", ""),
    ("国际化：日期格式应跟随系统区域", ""),
    ("依赖安全告警：tokio 版本需要升级", ""),
    ("测试：siliconflow_live 测试需要网络如何跳过", ""),
    ("文档：补充 cargo test -p ai-dispatcher 的运行说明", ""),
    ("代理 502 误报为 sandbox 权限错误的修复跟进", ""),
    ("希望把端口 8317 改为可配置", ""),
    ("版本 0.5.0-beta.14 迁移指南", ""),
    ("treat issue #2080 as duplicate of #1024", ""),
    ("commit 02063a9 之后日志记录格式变了", ""),
]


def run_self_test() -> tuple[float, bool]:
    """跑全部标注样本，返回 (准确率, 是否 >= 0.95)。

    准确率 = (正确判定的样本数) / (总样本数)，
    正确判定：trigger-positive 命中 blocked=True；compliant 命中 blocked=False。
    """
    assert len(TRIGGER_POSITIVE) >= 100, (
        f"trigger-positive 样本必须 >= 100，当前 {len(TRIGGER_POSITIVE)}"
    )
    assert len(COMPLIANT) >= 100, (
        f"compliant 样本必须 >= 100，当前 {len(COMPLIANT)}"
    )

    total = 0
    correct = 0
    misclassified: list[str] = []

    for title, body in TRIGGER_POSITIVE:
        total += 1
        res = classify(title, body)
        if res.blocked:
            correct += 1
        else:
            misclassified.append(f"[漏拦截] {title!r}")

    for title, body in COMPLIANT:
        total += 1
        res = classify(title, body)
        if not res.blocked:
            correct += 1
        else:
            misclassified.append(
                f"[误拦截/{res.classification}->{res.matched_terms}] {title!r}"
            )

    accuracy = correct / total if total else 0.0
    if misclassified:
        sys.stderr.write("误判样本：\n  " + "\n  ".join(misclassified) + "\n")
    return accuracy, accuracy >= 0.95


# ---------------------------------------------------------------------------
# pytest 用例
# ---------------------------------------------------------------------------
def test_sample_sizes():
    assert len(TRIGGER_POSITIVE) >= 100
    assert len(COMPLIANT) >= 100


def test_accuracy_at_least_95_percent():
    accuracy, ok = run_self_test()
    assert ok, f"准确率 {accuracy:.4f} 未达 spec §8 要求的 0.95"


def test_appeal_hint_always_present_on_block():
    """§5.5 兜底：任一命中结果都必须保留'申诉误判'入口。"""
    for title, body in TRIGGER_POSITIVE:
        res = classify(title, body)
        assert res.appeal_hint, f"命中结果缺少申诉入口: {title!r}"
        assert "申诉误判" in res.appeal_hint
        if res.blocked:
            assert res.label == "dispute-not-accepted"
            assert "申诉误判" in res.auto_reply  # §5.2 文案末行含申诉入口


def test_auto_reply_carries_spec_hotlines():
    res = classify("我的案子能赢吗", "")
    assert res.blocked
    assert "12348" in res.auto_reply
    assert "12351" in res.auto_reply
    assert "dispute-not-accepted" in res.auto_reply


def test_allowlist_avoids_false_positive_on_engineering_ids():
    """版本号 / 端口 / issue 编号 / hash 不应被个人信息正则误判。"""
    for title in (
        "升级到 v1.2.3 后配置文件不兼容",
        "服务监听 port 8317 时报地址被占用",
        "Issue #123 的修复未合并",
        "commit a04e54f 引入的回归",
    ):
        res = classify(title, "")
        assert not res.blocked, f"误拦截工程标识: {title!r} -> {res.classification}"


def test_no_emoji_in_auto_reply():
    for ch in classify("我的案子", "").auto_reply:
        c = ord(ch)
        is_emoji = (
            0x1F000 <= c <= 0x1FAFF
            or 0x2600 <= c <= 0x27BF
            or 0x1F1E6 <= c <= 0x1F1FF
            or c == 0xFE0F
        )
        assert not is_emoji, f"auto_reply 含 emoji U+{c:04X}"


if __name__ == "__main__":
    acc, passed = run_self_test()
    print(f"samples: trigger_positive={len(TRIGGER_POSITIVE)} compliant={len(COMPLIANT)}")
    print(f"accuracy={acc:.4f} threshold=0.95 pass={passed}")
    sys.exit(0 if passed else 1)
