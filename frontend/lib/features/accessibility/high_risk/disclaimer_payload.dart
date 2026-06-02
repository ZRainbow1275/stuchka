/// INV-10 high-risk scenarios — 7 triggers (spec 05 §5.7). Each carries a non-collapsible full
/// disclaimer and routes the user to legal-aid / union hotlines before any confirmation.
enum HighRiskScenario {
  resignAdvice,
  settlementBelow80,
  abandonClaim,
  groupRepresentativeAuth,
  criminalReportExport,
  protectedScenario,
  groupCreation,
}

extension HighRiskScenarioInfo on HighRiskScenario {
  String get title => switch (this) {
        HighRiskScenario.resignAdvice => '主动离职决定',
        HighRiskScenario.settlementBelow80 => '签署低于计算值 80% 的和解协议',
        HighRiskScenario.abandonClaim => '放弃仲裁请求项',
        HighRiskScenario.groupRepresentativeAuth => '群体案件代表授权',
        HighRiskScenario.criminalReportExport => '刑事报案材料导出',
        HighRiskScenario.protectedScenario => '特殊保护情形（医疗期 / 三期 / 未成年 / 工伤 / 性骚扰）',
        HighRiskScenario.groupCreation => '创建群组案件',
      };

  /// Full, non-collapsible disclaimer text (spec 05 §5.7.1 — 不允许折叠).
  String get fullDisclaimer => switch (this) {
        HighRiskScenario.resignAdvice =>
          '主动离职可能导致您无法主张经济补偿（《劳动合同法》第三十八条 / 第四十六条相关情形除外）。'
              '在确认前，请务必先核对：用人单位是否存在拖欠、未缴社保、未签合同等情形。'
              '本系统仅提供信息整理，不构成法律意见。建议先咨询法律援助或工会。',
        HighRiskScenario.settlementBelow80 =>
          '您即将签署的和解金额低于系统依据规则引擎计算结果的 80%。'
              '一旦签署并履行，通常意味着您放弃就差额部分再行主张的权利。'
              '请确认您已充分理解差额来源与放弃后果。本系统不替您决策。',
        HighRiskScenario.abandonClaim =>
          '放弃某一仲裁请求项后，通常不可在同一案件中再次主张。'
              '请确认该请求项确无证据支撑或确属您的自主选择。建议先咨询法律援助。',
        HighRiskScenario.groupRepresentativeAuth =>
          '作为群体案件代表，您的行为将影响其他被代表成员的权利。'
              '请确认您已获得全体被代表人的明确授权，并理解代表行为的法律后果。',
        HighRiskScenario.criminalReportExport =>
          '拒不支付劳动报酬罪的刑事报案材料导出属于 R1.5 功能，且需强制律师复核'
              '（§0529 prd §4.9.5）。当前版本仅提供引导，请联系法律援助或委托律师办理。',
        HighRiskScenario.protectedScenario =>
          '本情形涉及医疗期 / 孕期产期哺乳期 / 未成年 / 工伤 / 性骚扰等特殊保护场景，'
              '法律对用人单位的解除权有严格限制。请勿轻易接受不利安排，建议优先咨询专业法律援助。',
        HighRiskScenario.groupCreation =>
          '创建群组案件后，群组内成员的部分信息与进度将被共享。'
              '请确认所有成员知情同意，并理解群组数据的处理与授权链要求（§0529 prd §4.9.3）。',
      };
}
