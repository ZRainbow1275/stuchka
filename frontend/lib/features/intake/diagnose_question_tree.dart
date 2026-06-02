// M1 问诊树 (spec prd §4.1.4): a deterministic ≤7-question decision tree that drives the structured
// diagnosis. "系统每问只问一项；若用户跳过 ≥ 3 项则进入启发式追问模式（INV-08 三档低置信版本）".
//
// The tree is data-only and side-effect free; it does NOT itself decide the subcategory (that is the
// backend's deterministic DiagnosisEngine, item A). It only collects a normalized answer set
// (List<DiagnoseAnswer>) that POST /diagnose replays. Questions branch on identity so a 建筑劳务
// (农民工) intake asks wage/contract questions, a 新就业形态 intake asks the algorithm-control three
// factors (prd §4.1.4), etc.

import '../../ipc/dto/diagnose_dto.dart';
import '../../ipc/dto/enums.dart';

/// One selectable option in a question.
class DiagnoseOption {
  const DiagnoseOption({required this.id, required this.label, this.hint});
  final String id;
  final String label;
  final String? hint;
}

/// One 问诊 question (system asks exactly one thing per step — prd §4.1.4).
class DiagnoseQuestion {
  const DiagnoseQuestion({
    required this.id,
    required this.title,
    required this.options,
    this.subtitle,
    this.skippable = true,
  });

  final String id;
  final String title;
  final String? subtitle;
  final List<DiagnoseOption> options;
  final bool skippable;
}

/// The result of advancing the tree by one answer.
class DiagnoseTreeState {
  const DiagnoseTreeState({
    required this.answers,
    required this.skippedCount,
    required this.heuristicMode,
    required this.current,
    required this.askedCount,
  });

  final List<DiagnoseAnswer> answers;
  final int skippedCount;

  /// True once the user has skipped >= 3 questions (prd §4.1.4 启发式追问 / INV-08 low-confidence).
  final bool heuristicMode;

  /// The next question to render, or null when the tree is exhausted (ready to call /diagnose).
  final DiagnoseQuestion? current;
  final int askedCount;

  bool get isComplete => current == null;
}

/// The deterministic ≤7-question tree. Stateless: each step returns a fresh [DiagnoseTreeState].
class DiagnoseQuestionTree {
  DiagnoseQuestionTree({required this.identityType});

  final IdentityType identityType;

  /// Max questions per intake (prd §4.1.4 hard cap "≤ 7 问").
  static const int maxQuestions = 7;

  /// Skips >= this count flips into heuristic / low-confidence mode (prd §4.1.4 "跳过 >= 3 项").
  static const int heuristicSkipThreshold = 3;

  /// The ordered question bank for this identity. Built once; the engine walks it in order, capping
  /// at [maxQuestions]. Common questions (dispute area, evidence-on-hand, timing) apply to all
  /// identities; identity-specific questions are prepended.
  List<DiagnoseQuestion> get questions {
    final bank = <DiagnoseQuestion>[
      ..._identitySpecific(),
      ..._common(),
    ];
    return bank.length > maxQuestions ? bank.sublist(0, maxQuestions) : bank;
  }

  List<DiagnoseQuestion> _identitySpecific() {
    switch (identityType) {
      case IdentityType.newEmployment:
        // 新就业形态：补问"接单分配 / 路径监控 / 评分惩罚"三要素 (prd §4.1.4).
        return const [
          DiagnoseQuestion(
            id: 'algo_dispatch',
            title: '平台是否强制向您分配订单（不接单受处罚）？',
            subtitle: '新就业形态算法控制三要素之一：接单分配',
            options: [
              DiagnoseOption(id: 'yes', label: '是，强制派单'),
              DiagnoseOption(id: 'partial', label: '部分时段强制'),
              DiagnoseOption(id: 'no', label: '否，可自由接单'),
            ],
          ),
          DiagnoseQuestion(
            id: 'algo_route',
            title: '平台是否实时监控您的路径 / 在线时长？',
            subtitle: '三要素之二：路径监控',
            options: [
              DiagnoseOption(id: 'yes', label: '是，全程监控'),
              DiagnoseOption(id: 'no', label: '否'),
            ],
          ),
          DiagnoseQuestion(
            id: 'algo_score',
            title: '平台是否以评分 / 差评直接扣款或封号？',
            subtitle: '三要素之三：评分惩罚',
            options: [
              DiagnoseOption(id: 'yes', label: '是，评分影响收入'),
              DiagnoseOption(id: 'no', label: '否'),
            ],
          ),
        ];
      case IdentityType.constructionLabor:
        // 建筑劳务（农民工）：合同 + 工资发放主体 + 欠薪时长 (主用户路径 spec frontend/04 §4.9).
        return const [
          DiagnoseQuestion(
            id: 'wage_payer',
            title: '是谁直接给您发工资？',
            subtitle: '建工层级常见多层转包，认定用工主体是关键',
            options: [
              DiagnoseOption(id: 'company', label: '建筑公司 / 总包'),
              DiagnoseOption(id: 'foreman', label: '包工头 / 工长'),
              DiagnoseOption(id: 'labor_company', label: '劳务公司'),
              DiagnoseOption(id: 'unknown', label: '说不清'),
            ],
          ),
          DiagnoseQuestion(
            id: 'has_contract',
            title: '您是否签过书面劳动合同 / 劳务合同？',
            options: [
              DiagnoseOption(id: 'labor', label: '签了劳动合同'),
              DiagnoseOption(id: 'service', label: '签了劳务 / 承揽合同'),
              DiagnoseOption(id: 'none', label: '没签任何书面合同'),
            ],
          ),
        ];
      case IdentityType.retiredRehired:
        return const [
          DiagnoseQuestion(
            id: 'pension_status',
            title: '您是否已经依法领取养老金 / 退休金？',
            subtitle: '影响认定为劳动关系还是劳务关系',
            options: [
              DiagnoseOption(id: 'receiving', label: '已领取养老金'),
              DiagnoseOption(id: 'not_yet', label: '尚未领取'),
            ],
          ),
        ];
      default:
        return const [];
    }
  }

  List<DiagnoseQuestion> _common() {
    return const [
      DiagnoseQuestion(
        id: 'dispute_area',
        title: '您这次主要想解决的是什么问题？',
        subtitle: '只选最主要的一项；其他问题可在案件中陆续补充',
        skippable: false, // the dispute area anchors the whole diagnosis — not skippable.
        options: [
          DiagnoseOption(id: 'wage_arrears', label: '拖欠 / 克扣工资'),
          DiagnoseOption(id: 'illegal_termination', label: '违法解除 / 辞退'),
          DiagnoseOption(id: 'overtime', label: '加班费'),
          DiagnoseOption(id: 'social_insurance', label: '社保（欠缴 / 低缴 / 未缴）'),
          DiagnoseOption(id: 'work_injury', label: '工伤'),
          DiagnoseOption(id: 'no_contract', label: '未签合同 / 二倍工资'),
          DiagnoseOption(id: 'other', label: '其他 / 不确定'),
        ],
      ),
      DiagnoseQuestion(
        id: 'relationship_proof',
        title: '您手上能证明"在这里上班"的材料有哪些？',
        subtitle: '用于评估劳动关系链是否闭合',
        options: [
          DiagnoseOption(id: 'contract', label: '劳动合同'),
          DiagnoseOption(id: 'payroll', label: '工资条 / 银行流水'),
          DiagnoseOption(id: 'chat', label: '微信 / 工作群记录'),
          DiagnoseOption(id: 'witness', label: '工友可以作证'),
          DiagnoseOption(id: 'none', label: '暂时都没有'),
        ],
      ),
      DiagnoseQuestion(
        id: 'still_employed',
        title: '您现在还在这家单位工作吗？',
        options: [
          DiagnoseOption(id: 'yes', label: '仍在职'),
          DiagnoseOption(id: 'left', label: '已离职 / 被辞退'),
        ],
      ),
      DiagnoseQuestion(
        id: 'amount_band',
        title: '您预估涉及的金额大概在哪个区间？',
        subtitle: '用于初步评估程序选择（金额大小影响主路径）',
        options: [
          DiagnoseOption(id: 'lt_1w', label: '1 万元以下'),
          DiagnoseOption(id: '1w_5w', label: '1 万 – 5 万元'),
          DiagnoseOption(id: 'gt_5w', label: '5 万元以上'),
          DiagnoseOption(id: 'unknown', label: '还算不清'),
        ],
      ),
    ];
  }

  /// Walk the answer list to the next unanswered question, computing skip count + heuristic mode.
  DiagnoseTreeState advance(List<DiagnoseAnswer> answers) {
    final byId = {for (final a in answers) a.questionId: a};
    final skipped = answers.where((a) => a.skipped).length;
    DiagnoseQuestion? next;
    var asked = 0;
    for (final q in questions) {
      asked++;
      if (!byId.containsKey(q.id)) {
        next = q;
        asked--; // the current question is not yet answered.
        break;
      }
    }
    return DiagnoseTreeState(
      answers: answers,
      skippedCount: skipped,
      heuristicMode: skipped >= heuristicSkipThreshold,
      current: next,
      askedCount: asked,
    );
  }
}
