/// INV-10 high-risk scenarios — 7 triggers (compliance/05 §1 S-01..S-06 + spec 05 §5.7 群组创建).
///
/// This is the SINGLE canonical home for the high-risk scenario enum, its stable compliance
/// `scene_id`, its title, and its VERBATIM full disclaimer text. The disclaimer arms are copied
/// field-for-field from compliance/05 §3.1-§3.6 (truth source = prompts/0529); every arm is
/// >= 200 chars and carries the interpolation params the spec defines. The text is hard-coded and
/// MUST NOT be paraphrased or user-edited (compliance/05 §3 "禁止任何形式的修改（含标点）").
///
/// BYTE-VERBATIM contract (compliance/05 §3 "禁止任何形式的修改（含标点）"):
///   - Punctuation is byte-exact to the spec, including its ASCII straight double quotes (`"…"`,
///     NOT typographic `“…”`). The spec's `### 3.x` fenced blocks use ASCII straight quotes only.
///   - Line breaks and indentation are byte-exact: the spec's blank lines (`\n\n`) and its
///     3-space / 5-space sub-bullet indentation are reproduced literally (each spec source line is
///     one `'…\n'` literal segment here).
///   - The `{slot}` placeholders the spec defines stay live ({expected_amount} → `${...}`, etc.).
///   - The only normalisation applied is stripping the spec's Markdown emphasis markers (`**…**`):
///     those are Markdown rendering of the .md document, not legal prose — rendering literal `**`
///     to a user in a disclaimer dialog would be a defect. Quotes/line-breaks/indentation (the
///     punctuation the lawyer review locks) are preserved exactly. The 7th arm (`groupCreation`)
///     is the project's own §5.7 / prd §4.9.3 template, NOT a spec §3 block, so it is not bound by
///     §3 byte-verbatim; its quotes are kept ASCII-straight for consistency with the §3 arms.
library;

/// The 7 high-risk decision scenarios (compliance/05 §1 + spec 05 §5.7).
enum HighRiskScenario {
  /// S-01 主动离职（基于系统建议）.
  resignAdvice,

  /// S-02 签和解协议（金额 < 计算值 80%）.
  settlementBelow80,

  /// S-03 放弃任一仲裁请求项.
  abandonClaim,

  /// S-04 群体案件代表授权.
  groupRepresentativeAuth,

  /// S-05 刑事报案材料导出（拒不支付劳动报酬罪）.
  criminalReportExport,

  /// S-06 医疗期 / 三期 / 未成年 / 工伤 / 性骚扰场景的关键决策.
  protectedScenario,

  /// 群组创建（spec 05 §5.7 第 7 类 / §0529 prd §4.9.3）.
  groupCreation,
}

/// Interpolation params carried into the verbatim templates (compliance/05 §3.2/§3.3/§3.4).
///
/// The spec fixes the disclaimer prose; only the bracketed `{...}` slots are filled at call sites.
/// Unsupplied slots fall back to a neutral "（未提供）" so the >= 200-char floor never depends on
/// runtime data.
class DisclaimerPayload {
  const DisclaimerPayload({
    this.expectedAmount,
    this.settlementAmount,
    this.ratio,
    this.requestedItemName,
    this.amount,
    this.confidence,
    this.grantOrAccept,
    this.roleSpecificDescription,
    this.detectedCategories,
  });

  /// S-02 {expected_amount} — 规则引擎计算的应得金额.
  final String? expectedAmount;

  /// S-02 {settlement_amount} — 当前和解金额.
  final String? settlementAmount;

  /// S-02 {ratio} — 和解金额占应得金额的百分比（整数，不含 %）.
  final String? ratio;

  /// S-03 {requested_item_name} — 被放弃的请求项名称.
  final String? requestedItemName;

  /// S-03 {amount} — 该请求项对应主张金额.
  final String? amount;

  /// S-03 {confidence} — 规则引擎对其胜诉概率的评估.
  final String? confidence;

  /// S-04 {grant_or_accept} — "接受" 或 "给出".
  final String? grantOrAccept;

  /// S-04 {role_specific_description} — 角色专属说明（被代表方 / 代表人）.
  final String? roleSpecificDescription;

  /// S-06 {detected_categories} — 命中的特殊保护类别（如 "医疗期、工伤"）.
  final String? detectedCategories;

  static const DisclaimerPayload none = DisclaimerPayload();
}

/// Which INV-10 scenarios are wired to a REAL trigger site in this build vs deferred to their
/// owning module (compliance/05 §2.2 触发协议). This is the explicit "no silent drop" record the
/// task mandates: the gate IS the same component everywhere; only the call site differs.
///
/// Wired now (real user actions in this build):
///   - S-01 resignAdvice           → 诊断结论页 (diagnose_page.dart "我考虑按系统提示主动离职")
///   - S-02 settlementBelow80       → M9 和解 (comp_calc_page.dart "签署和解协议", < 80% branch)
///   - S-03 abandonClaim            → 文书宿主 (document_host_page.dart "放弃某项仲裁请求")
///   - S-05 criminalReportExport    → 文书宿主导出 (document_host_page.dart "导出刑事报案材料")
///
/// Deferred to their owning module (no business flow yet in this build):
///   - S-04 groupRepresentativeAuth → 群体案件代表授权 / 授权委托书生成 (群体案件族, not yet built)
///   - groupCreation                → 群组创建 (群体案件族, §0529 prd §4.9.3, not yet built)
const Set<HighRiskScenario> kWiredHighRiskScenarios = {
  HighRiskScenario.resignAdvice,
  HighRiskScenario.settlementBelow80,
  HighRiskScenario.abandonClaim,
  HighRiskScenario.criminalReportExport,
};

/// The scenarios whose REAL trigger site is deferred to a later (group-case) module.
const Set<HighRiskScenario> kDeferredHighRiskScenarios = {
  HighRiskScenario.groupRepresentativeAuth,
  HighRiskScenario.groupCreation,
};

extension HighRiskScenarioInfo on HighRiskScenario {
  /// The compliance/05 §1 stable id used by the audit `scene_id` field (§2.3).
  String get sceneId => switch (this) {
        HighRiskScenario.resignAdvice => 'S-01',
        HighRiskScenario.settlementBelow80 => 'S-02',
        HighRiskScenario.abandonClaim => 'S-03',
        HighRiskScenario.groupRepresentativeAuth => 'S-04',
        HighRiskScenario.criminalReportExport => 'S-05',
        HighRiskScenario.protectedScenario => 'S-06',
        HighRiskScenario.groupCreation => 'GROUP-CREATE',
      };

  String get title => switch (this) {
        HighRiskScenario.resignAdvice => '主动离职决定',
        HighRiskScenario.settlementBelow80 => '签署低于计算值 80% 的和解协议',
        HighRiskScenario.abandonClaim => '放弃仲裁请求项',
        HighRiskScenario.groupRepresentativeAuth => '群体案件代表授权',
        HighRiskScenario.criminalReportExport => '刑事报案材料导出',
        HighRiskScenario.protectedScenario => '特殊保护情形（医疗期 / 三期 / 未成年 / 工伤 / 性骚扰）',
        HighRiskScenario.groupCreation => '创建群组案件',
      };

  /// Full, non-collapsible disclaimer text, VERBATIM from compliance/05 §3.1-§3.6 (>= 200 chars).
  /// [payload] fills the `{...}` interpolation slots the spec defines; missing slots fall back to
  /// "（未提供）" so the char floor never depends on runtime data.
  String fullDisclaimer([DisclaimerPayload payload = DisclaimerPayload.none]) {
    String slot(String? v) => (v == null || v.isEmpty) ? '（未提供）' : v;
    switch (this) {
      case HighRiskScenario.resignAdvice:
        // compliance/05 §3.1 S-01 · 主动离职 (byte-verbatim — see the library doc comment).
        return '重要提示：关于您当前考虑的"主动离职"决定\n'
            '\n'
            '本系统识别到您正在考虑主动离职。请您在按下确认前充分理解：\n'
            '\n'
            '1. 一旦您主动以"个人原因" 提出离职，依据《劳动合同法》第三十六条，\n'
            '   您将不再有权依据本法第四十六条、第四十七条要求经济补偿金；\n'
            '   即便用人单位先前存在违法行为（如拖欠工资、未缴社保），\n'
            '   主动离职的方式将显著削弱您事后追偿的法律地位。\n'
            '\n'
            '2. 本系统基于您输入的事实给出的"建议主动离职" 提示，\n'
            '   仅是基于您当前描述的争议子类与赔偿计算的概率性参考，\n'
            '   不构成法律意见。事实如有不同（如您实际持有更强证据、\n'
            '   或可主张被迫离职），结论可能完全相反。\n'
            '\n'
            '3. 推荐的替代路径：\n'
            '   - 以"被迫解除劳动合同" 为由（《劳动合同法》第三十八条）提出解除，\n'
            '     并在通知中明确列举用人单位的违法事实，\n'
            '     这可同时保留经济补偿金的索赔权利；\n'
            '   - 在做出任何决定前，先联系全国法律援助热线 12348、\n'
            '     全国总工会维权热线 12351，或当地工会 / 法援中心，\n'
            '     获得免费的一对一人工法律意见；\n'
            '   - 若您身处医疗期、三期或刚发生工伤，本系统强烈建议您暂缓任何\n'
            '     形式的离职决定，先与上述渠道核实您当前的法定权益。\n'
            '\n'
            '4. 本系统的反 HR 立场是道德姿态，不是技术防御。\n'
            '   本系统对您基于本提示做出的任何决定不承担法律责任。\n'
            '\n'
            '参考资源：\n'
            '- 全国法律援助热线：12348\n'
            '- 全国总工会维权热线：12351\n'
            '- 心理援助专线：12320 / 12351 中的心理咨询通道\n'
            '- 中华全国律师协会公益法援平台\n'
            '\n'
            '如您仍坚持主动离职，请在 8 秒冷静期结束后二次确认。';
      case HighRiskScenario.settlementBelow80:
        // compliance/05 §3.2 S-02 · 签和解协议（金额 < 计算值 80%）(byte-verbatim).
        return '重要提示：关于您当前考虑签署的和解协议\n'
            '\n'
            '本系统的规则引擎计算结果显示：依据您输入的事实，\n'
            '您应得金额约为 ¥ ${slot(payload.expectedAmount)}（详细分项见上方表格）。\n'
            '当前和解协议金额为 ¥ ${slot(payload.settlementAmount)}，\n'
            '约为应得金额的 ${slot(payload.ratio)} %。\n'
            '\n'
            '请您在签署前充分理解：\n'
            '\n'
            '1. 和解协议一旦签署、用人单位履行完毕，\n'
            '   依据《最高人民法院关于审理劳动争议案件适用法律若干问题的解释》，\n'
            '   一般情况下不得就同一争议再行起诉，\n'
            '   即您将失去通过仲裁 / 诉讼追加补足的法律途径。\n'
            '\n'
            '2. 本系统的规则引擎计算结果仅是基于您输入事实的确定性计算，\n'
            '   不包括以下不确定因素：\n'
            '   - 用人单位实际偿付能力（即便胜诉也可能无力执行）；\n'
            '   - 仲裁 / 诉讼周期（一般 6-18 个月，期间不发工资）；\n'
            '   - 您的精力、心理与时间成本；\n'
            '   - 个案的程序性风险（举证不充分、时效已过等）。\n'
            '   上述因素可能使您接受当前和解金额合理；但请由您自己评估，\n'
            '   而不是由本系统替您评估。\n'
            '\n'
            '3. 强烈建议您在签字前：\n'
            '   - 联系 12348 法援、12351 工会或当地法律援助中心，\n'
            '     获得免费的一对一人工法律意见；\n'
            '   - 至少征求一位执业律师的意见（多数律师对劳动争议提供免费简短咨询）；\n'
            '   - 如和解金额涉及一次性买断社保、医疗期、工伤等敏感权益，\n'
            '     额外要求律师审阅协议条款；\n'
            '   - 拒绝任何"现场签字、不给带回家研究" 的施压。\n'
            '\n'
            '4. 本系统的反 HR 立场是道德姿态，不是技术防御。\n'
            '   本系统对您基于本提示做出的任何和解决定不承担法律责任。\n'
            '\n'
            '参考资源：\n'
            '- 全国法律援助热线：12348\n'
            '- 全国总工会维权热线：12351\n'
            '- 当地法律援助中心（工会、司法局下属）\n'
            '- 中华全国律师协会公益法援平台\n'
            '\n'
            '如您仍决定签署当前和解协议，请在 8 秒冷静期结束后二次确认。';
      case HighRiskScenario.abandonClaim:
        // compliance/05 §3.3 S-03 · 放弃任一仲裁请求项 (byte-verbatim).
        return '重要提示：关于您当前考虑放弃的仲裁请求项\n'
            '\n'
            '您正在考虑从仲裁请求中删除以下请求项：\n'
            '"${slot(payload.requestedItemName)}"\n'
            '（本项目对应主张金额：¥ ${slot(payload.amount)}；\n'
            '  规则引擎对其胜诉概率评估为：${slot(payload.confidence)}）\n'
            '\n'
            '请您在确认前理解：\n'
            '\n'
            '1. 一旦放弃，该请求项不能在同一仲裁程序中再次提出。\n'
            '   即便您后续发现新的证据，也需要通过补充申请或单独起诉等方式重新主张，\n'
            '   程序成本显著增加。\n'
            '\n'
            '2. 本系统对"胜诉概率" 的评估基于您输入的事实与当前证据，\n'
            '   不代表确定结论。事实如有补充（如新增证据、补充陈述），\n'
            '   评估结果可能反转。\n'
            '\n'
            '3. 放弃请求项的常见原因有：\n'
            '   - 节省仲裁费 / 应诉精力 → 仲裁费通常很低（一般 ≤ 数百元），\n'
            '     不应作为放弃实体权利的理由；\n'
            '   - 担心举证不足 → 举证不足时建议保留请求项 + 在仲裁庭上申请调查取证，\n'
            '     而不是直接放弃；\n'
            '   - 被对方"以撤销其他主张为条件诱导" → 这通常对您不利，\n'
            '     建议先咨询人工法律意见。\n'
            '\n'
            '4. 强烈建议您在确认前联系：\n'
            '   - 全国法律援助热线 12348；\n'
            '   - 全国总工会维权热线 12351；\n'
            '   - 当地法律援助中心。\n'
            '\n'
            '5. 本系统的反 HR 立场是道德姿态，不是技术防御。\n'
            '   本系统对您基于本提示放弃请求项的后果不承担法律责任。\n'
            '\n'
            '如您仍决定放弃该请求项，请在 8 秒冷静期结束后二次确认。';
      case HighRiskScenario.groupRepresentativeAuth:
        // compliance/05 §3.4 S-04 · 群体案件代表授权 (byte-verbatim).
        return '重要提示：关于您当前考虑接受 / 给出的群体案件代表授权\n'
            '\n'
            '您正在 ${slot(payload.grantOrAccept)} 群体案件代表授权。\n'
            '${slot(payload.roleSpecificDescription)}\n'
            '\n'
            '请您在确认前理解：\n'
            '\n'
            '1. 群体案件中，代表人有权代被代表的劳动者：\n'
            '   - 提交、撤回、变更仲裁/诉讼请求；\n'
            '   - 接受和解、调解；\n'
            '   - 处置部分或全部诉讼权利（如放弃请求项、降低主张金额）；\n'
            '   依据《民事诉讼法》代表人诉讼条款，\n'
            '   代表人的处置行为对全体被代表人发生法律效力，\n'
            '   除非代表人事先取得全体被代表人的明确授权。\n'
            '\n'
            '2. 作为被代表方，您：\n'
            '   - 一旦签字，对代表人合理范围内的程序处置受其约束；\n'
            '   - 仍可单独委托律师；\n'
            '   - 有权随时书面撤回授权（建议同时通知仲裁/法院，否则程序上未必生效）。\n'
            '\n'
            '3. 作为代表人，您：\n'
            '   - 必须保管好每位被代表人的明确授权书；\n'
            '   - 任何"和解 / 放弃请求 / 降低主张" 的处置，\n'
            '     建议事先取得每位被代表人的书面同意（不仅是默示授权）；\n'
            '   - 群体内出现"疑似雇主代理人" 时，应及时与工会 / 法援沟通处置。\n'
            '\n'
            '4. 本系统的反 HR 立场是道德姿态，不是技术防御。\n'
            '   本系统不能识别群体中是否存在卧底或敌意成员，\n'
            '   请您只与可信任的同事共享案件信息。\n'
            '\n'
            '5. 强烈建议在签署前：\n'
            '   - 联系工会 12351 寻求群体案件指导；\n'
            '   - 联系法律援助 12348；\n'
            '   - 必要时聘请专业律师（多数地市存在劳动法专项援助律师）。\n'
            '\n'
            '参考资源：\n'
            '- 全国法律援助热线：12348\n'
            '- 全国总工会维权热线：12351\n'
            '- 当地工会群体案件维权指导\n'
            '- 中华全国律师协会公益法援平台\n'
            '\n'
            '如您仍决定 ${slot(payload.grantOrAccept)} 该授权，请在 8 秒冷静期结束后二次确认。';
      case HighRiskScenario.criminalReportExport:
        // compliance/05 §3.5 S-05 · 刑事报案材料导出（拒不支付劳动报酬罪）(byte-verbatim).
        return '重要提示：关于您当前考虑导出的刑事报案材料\n'
            '\n'
            '您正在导出针对"拒不支付劳动报酬罪"（《刑法》第二百七十六条之一）\n'
            '的报案材料。\n'
            '\n'
            '请您在导出前充分理解：\n'
            '\n'
            '1. 拒不支付劳动报酬罪是严肃的刑事指控，\n'
            '   报案材料一旦提交公安机关，将启动刑事侦查程序，\n'
            '   被指控的单位负责人可能被采取强制措施。\n'
            '   该决定不可轻易撤销，对被告方法律后果严重。\n'
            '\n'
            '2. 本罪有严格的构成要件：\n'
            '   - 主体特定（用人单位的实际负责人或直接责任人员）；\n'
            '   - 客观行为（以转移财产、逃匿等方法逃避支付，\n'
            '     或者有能力支付而不支付）；\n'
            '   - 数额较大（参考最高检/最高法司法解释，通常以欠薪金额 + 人数为参考）；\n'
            '   - 经政府有关部门责令支付仍不支付（前置程序）。\n'
            '   缺少前置程序（如未先经劳动监察大队责令支付）时报案，\n'
            '   公安机关通常不予立案。\n'
            '\n'
            '3. 强烈建议您在导出前：\n'
            '   - 先经劳动监察大队（劳动保障监察部门）投诉并取得"责令支付通知书"，\n'
            '     这是大多数地区刑事立案的前置要件；\n'
            '   - 咨询律师评估是否构成本罪，避免被反控诬告陷害（《刑法》第 243 条）；\n'
            '   - 同步保留民事维权路径（劳动仲裁 + 诉讼 + 强制执行），\n'
            '     刑事程序不能直接替代民事追偿。\n'
            '\n'
            '4. 本系统生成的报案材料仅是基于您描述事实的辅助文书，\n'
            '   不构成对本案是否构成刑事犯罪的法律判断。\n'
            '   您的实际报案行为是否被立案、是否合法，由您自行承担后果。\n'
            '\n'
            '5. 本系统的反 HR 立场是道德姿态，不是技术防御。\n'
            '   本系统不能保证您的报案行为不被对方反诉。\n'
            '\n'
            '参考资源：\n'
            '- 全国法律援助热线：12348\n'
            '- 全国总工会维权热线：12351\n'
            '- 全国劳动保障监察投诉举报：12333\n'
            '- 当地公安经济犯罪侦查部门（咨询前置程序）\n'
            '\n'
            '如您已完成前置程序，并仍决定导出报案材料，请在 8 秒冷静期结束后二次确认。';
      case HighRiskScenario.protectedScenario:
        // compliance/05 §3.6 S-06 · 医疗期 / 三期 / 未成年 / 工伤 / 性骚扰 (byte-verbatim).
        return '重要提示：关于您当前所处场景的特殊保护\n'
            '\n'
            '本系统识别到您当前案件涉及以下一类或多类特殊保护情形：\n'
            '${slot(payload.detectedCategories)}\n'
            '（其中：医疗期、三期 = 孕期 / 产期 / 哺乳期、未成年劳动者、工伤、性骚扰）\n'
            '\n'
            '请您在做出任何关键决策（离职、签字、放弃请求项、撤诉等）前理解：\n'
            '\n'
            '1. 上述情形受多部法律的额外保护，\n'
            '   常规的"劳动争议处理路径" 在这些场景下存在重要差异：\n'
            '   - 医疗期：用人单位不得在医疗期内以《劳动合同法》第四十条解除合同；\n'
            '   - 三期：用人单位原则上不得解除合同，违反者赔偿金按 2N 计算；\n'
            '   - 未成年：禁止用工年龄 16 周岁以下；16-18 周岁未成年工享有特殊保护；\n'
            '   - 工伤：先做工伤认定，再做劳动能力鉴定，\n'
            '     程序不可跳过；待遇标准按《工伤保险条例》执行；\n'
            '   - 性骚扰：除《民法典》第一千零一十条侵权救济外，\n'
            '     用人单位负有预防与制止义务（《妇女权益保障法》第二十五条）。\n'
            '\n'
            '2. 在这些场景中，本系统的规则引擎与 AI 建议有更高的不确定性：\n'
            '   - 涉及医疗 / 鉴定的部分依赖鉴定机构判断；\n'
            '   - 涉及未成年与性骚扰的部分常含程序之外的伦理与心理支持需求；\n'
            '   - 涉及三期的部分依赖具体地方政策。\n'
            '   本系统对这些场景的建议仅作参考，不构成法律意见。\n'
            '\n'
            '3. 本系统强烈建议您不要在这些场景中仅凭本系统的建议做出关键决策：\n'
            '   - 联系全国法律援助热线 12348 申请免费法援律师；\n'
            '   - 联系全国总工会维权热线 12351；\n'
            '   - 性骚扰场景：可同时联系全国妇联 12338；\n'
            '   - 未成年场景：可联系未成年人保护热线 12345 / 当地未保办；\n'
            '   - 工伤场景：联系当地工伤保险经办机构与劳动监察大队；\n'
            '   - 心理压力大时：12320 / 12351 心理咨询通道。\n'
            '\n'
            '4. 本系统的反 HR 立场是道德姿态，不是技术防御。\n'
            '   本系统对您基于本提示做出的任何决定不承担法律责任。\n'
            '\n'
            '参考资源：\n'
            '- 全国法律援助热线：12348\n'
            '- 全国总工会维权热线：12351\n'
            '- 全国妇联维权热线：12338（性骚扰、三期）\n'
            '- 未成年人保护：12345 / 当地未保办（未成年劳动者）\n'
            '- 劳动保障监察：12333\n'
            '- 心理援助：12320 / 12351 心理咨询通道\n'
            '\n'
            '如您已知悉上述特殊保护并仍决定继续当前操作，请在 8 秒冷静期结束后二次确认。';
      case HighRiskScenario.groupCreation:
        // 群组创建 (spec 05 §5.7 第 7 类 / §0529 prd §4.9.3). 群体案件代表授权文案前置的本系统侧告知，
        // 复用 S-04 群体处置的核心风险并补充数据共享 / 授权链要点（compliance/05 §8: 与 INV-09 立场弹窗串行）。
        return '重要提示：关于您当前考虑创建的群组案件\n\n'
            '您正在创建一个群组案件。群组创建后，群组内成员的部分案件信息与办理进度'
            '将在群组授权链范围内被共享，且后续可能进入"群体案件代表授权"（见单独提示）。\n\n'
            '请您在创建前充分理解：\n\n'
            '1. 群组案件一旦建立，成员之间将共享案件事实、证据目录与进度状态。'
            '请确认所有拟加入成员均已知情同意，并理解群组数据的处理与授权链要求'
            '（§0529 prd §4.9.3）。\n\n'
            '2. 群体维权可显著增强谈判与举证地位，但也带来协调成本与信息暴露风险：\n'
            '   - 群组内任何成员的处置（和解、撤回、放弃请求项）都可能影响群体策略；\n'
            '   - 本系统无法识别群体中是否存在卧底或雇主代理人，'
            '请只与可信任的同事共享案件信息；\n'
            '   - 群体案件代表授权将以单独的完整免责弹窗串行触发，请勿在未阅读时授权。\n\n'
            '3. 强烈建议您在创建群组前：\n'
            '   - 联系全国总工会维权热线 12351 寻求群体案件指导；\n'
            '   - 联系全国法律援助热线 12348；\n'
            '   - 必要时聘请专业劳动法律师统一代理。\n\n'
            '4. 本系统的反 HR 立场是道德姿态，不是技术防御。'
            '本系统对您基于本提示创建群组并共享信息的后果不承担法律责任。\n\n'
            '参考资源：\n'
            '- 全国法律援助热线：12348\n'
            '- 全国总工会维权热线：12351\n'
            '- 当地工会群体案件维权指导\n'
            '- 中华全国律师协会公益法援平台\n\n'
            '如您仍决定创建该群组案件，请在 8 秒冷静期结束后二次确认。';
    }
  }
}
