"""上线门槛常量 — the single source of truth for the §5.3 gates (ai/05 + prd/06 §6.2).

The Rust oracle never judges thresholds; these constants are the only place the gates are decided.
"""

# Automated, REQUIRED (block release on miss):
CALC_50_PASS_RATE_MIN = 1.0       # 计算正确率 100% (M9, 错一道不发版)
DEADLINE_30_PASS_RATE_MIN = 1.0   # 时效正确率 100% (M5, 错一道不发版; 06 §6.9)
GB45438_COMPLETENESS_MIN = 1.0    # doc-20 四层标识完整性 100% (INV-02)
PII_200_RECALL_MIN = 0.95         # 高敏召回 >= 95%
PII_200_FP_RATE_MAX = 0.05        # 高敏误报 <= 5%
LAW_200_STRUCTURAL_MIN = 1.0      # 法条 URN 结构有效性 100% (无幻觉/畸形 law id)

# Human process gates (reported, NOT auto-decided):
LAW_200_ACCURACY_MIN = 0.95       # 法条准确率 >= 95% (律师盲测)
DOC_20_USABLE_RATE_MIN = 0.70     # 文书可用率 >= 70% (3 律师盲测)
FACT_30_VERIFIED_RATE_MIN = 0.80  # 事实核验率 >= 80% (5 内部审阅)

# Deferred to R1b (ai/05 §5.12):
ABSTENTION_REFUSAL_RATE_MAX = 0.10  # 拒答率 <= 10%
ABSTENTION_BUCKET_HIT_MIN = 0.90    # 桶命中 >= 90%
PATH_DURATION_MIN_MINUTES = 30
PATH_DURATION_MAX_MINUTES = 60
