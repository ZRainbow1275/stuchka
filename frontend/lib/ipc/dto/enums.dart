// Dart mirror enums for the backend data-model wire contract.
//
// Every value below MUST equal the backend `#[serde(rename_all = "snake_case")]` wire value
// (Stučka/backend/crates/data-model/src/enums.rs + pii.rs). The round-trip is asserted in
// test/dto/enum_roundtrip_test.dart against fixtures lifted verbatim from the Rust tests.

/// A serde-snake_case enum mirror with explicit wire values.
mixin _WireEnum {
  String get wire;
}

/// IdentityType — 11 forced single-choice (data-model enums.rs::IdentityType).
enum IdentityType implements _WireEnum {
  standardFullTime('standard_full_time'),
  dispatch('dispatch'),
  partTime('part_time'),
  newEmployment('new_employment'),
  domesticService('domestic_service'),
  constructionLabor('construction_labor'),
  individualEmployee('individual_employee'),
  intern('intern'),
  retiredRehired('retired_rehired'),
  contractor('contractor'),
  deFactoNoContract('de_facto_no_contract');

  const IdentityType(this.wire);
  @override
  final String wire;

  String get labelZh => switch (this) {
        standardFullTime => '城镇标准全日制',
        dispatch => '劳务派遣',
        partTime => '非全日制',
        newEmployment => '网约车 / 外卖 / 即时配送',
        domesticService => '家政 / 月嫂 / 钟点工',
        constructionLabor => '建筑劳务（农民工）',
        individualEmployee => '个体工商户雇员',
        intern => '实习生 / 学徒',
        retiredRehired => '退休返聘',
        contractor => '承包 / 承揽 / 自由职业',
        deFactoNoContract => '未签合同事实劳动',
      };

  static IdentityType fromWire(String w) =>
      values.firstWhere((e) => e.wire == w, orElse: () => standardFullTime);
}

/// DisputeSubtype — 8 social-insurance branches (data-model enums.rs::DisputeSubtype).
///
/// NOTE (brief §6.4): the backend currently only models the 8 social-insurance subtypes; the
/// 农民工欠薪 (WAGE_ARREARS) main path is not yet a backend enum value. `IntakePage` renders the
/// real backend enum; WAGE_ARREARS stays a placeholder until data-model extends the enum.
enum DisputeSubtype implements _WireEnum {
  socialInsWaiverInvalid('social_ins_waiver_invalid'),
  socialInsArrears('social_ins_arrears'),
  socialInsUnderpaidBase('social_ins_underpaid_base'),
  socialInsIntermittent('social_ins_intermittent'),
  socialInsProxy('social_ins_proxy'),
  socialInsUninsuredInjury('social_ins_uninsured_injury'),
  socialInsCrossPeriod('social_ins_cross_period'),
  socialInsBaseDispute('social_ins_base_dispute');

  const DisputeSubtype(this.wire);
  @override
  final String wire;

  String get labelZh => switch (this) {
        socialInsWaiverInvalid => '弃缴约定无效 + 单方解除',
        socialInsArrears => '社保普通欠缴',
        socialInsUnderpaidBase => '低缴',
        socialInsIntermittent => '漏缴',
        socialInsProxy => '挂靠社保',
        socialInsUninsuredInjury => '未参保导致工伤损失',
        socialInsCrossPeriod => '跨期争议',
        socialInsBaseDispute => '基数缴费争议',
      };

  static DisputeSubtype fromWire(String w) =>
      values.firstWhere((e) => e.wire == w, orElse: () => socialInsArrears);
}

enum CoverageTier implements _WireEnum {
  makeDeep('make_deep'),
  makeUsable('make_usable');

  const CoverageTier(this.wire);
  @override
  final String wire;

  static CoverageTier fromWire(String w) =>
      values.firstWhere((e) => e.wire == w, orElse: () => makeUsable);
}

enum CaseStatus implements _WireEnum {
  draft('draft'),
  diagnosed('diagnosed'),
  confirmed('confirmed'),
  frozen('frozen'),
  disputed('disputed');

  const CaseStatus(this.wire);
  @override
  final String wire;

  static CaseStatus fromWire(String w) =>
      values.firstWhere((e) => e.wire == w, orElse: () => draft);
}

enum FactCategory implements _WireEnum {
  relationQualification('relation_qualification'),
  wage('wage'),
  timePeriod('time_period'),
  terminationReason('termination_reason'),
  workInjury('work_injury'),
  discrimination('discrimination'),
  other('other');

  const FactCategory(this.wire);
  @override
  final String wire;

  static FactCategory fromWire(String w) =>
      values.firstWhere((e) => e.wire == w, orElse: () => other);
}

enum FactStatus implements _WireEnum {
  pending('pending'),
  confirmed('confirmed'),
  disputed('disputed'),
  deprecated('deprecated');

  const FactStatus(this.wire);
  @override
  final String wire;

  static FactStatus fromWire(String w) =>
      values.firstWhere((e) => e.wire == w, orElse: () => pending);
}

enum FactSource implements _WireEnum {
  userInput('user_input'),
  aiInferred('ai_inferred'),
  ruleEngine('rule_engine');

  const FactSource(this.wire);
  @override
  final String wire;

  static FactSource fromWire(String w) =>
      values.firstWhere((e) => e.wire == w, orElse: () => userInput);
}

/// EvidenceCategory — 7 classes (data-model enums.rs::EvidenceCategory).
enum EvidenceCategory implements _WireEnum {
  documentaryContract('documentary_contract'),
  audioVideo('audio_video'),
  digitalCommunication('digital_communication'),
  witnessStatement('witness_statement'),
  scenePhotoVideo('scene_photo_video'),
  thirdPartyData('third_party_data'),
  appraisal('appraisal');

  const EvidenceCategory(this.wire);
  @override
  final String wire;

  String get labelZh => switch (this) {
        documentaryContract => '书证 / 合同',
        audioVideo => '录音 / 录像',
        digitalCommunication => '电子聊天 / 通讯记录',
        witnessStatement => '证人证言',
        scenePhotoVideo => '现场照片 / 视频',
        thirdPartyData => '银行流水 / 税单 / 社保',
        appraisal => '鉴定 / 评估',
      };

  static EvidenceCategory fromWire(String w) =>
      values.firstWhere((e) => e.wire == w, orElse: () => documentaryContract);
}

enum EvidenceStatus implements _WireEnum {
  uploaded('uploaded'),
  parsed('parsed'),
  scored('scored'),
  verified('verified'),
  disputed('disputed'),
  quarantined('quarantined');

  const EvidenceStatus(this.wire);
  @override
  final String wire;

  static EvidenceStatus fromWire(String w) =>
      values.firstWhere((e) => e.wire == w, orElse: () => uploaded);
}

enum ClaimType implements _WireEnum {
  economicCompensation('economic_compensation'),
  economicDamage('economic_damage'),
  doubleWageNoContract('double_wage_no_contract'),
  overtimePay('overtime_pay'),
  maliciousArrearsSurcharge('malicious_arrears_surcharge'),
  workInjuryBenefit('work_injury_benefit'),
  other('other');

  const ClaimType(this.wire);
  @override
  final String wire;

  static ClaimType fromWire(String w) =>
      values.firstWhere((e) => e.wire == w, orElse: () => other);
}

enum ClaimStatus implements _WireEnum {
  draft('draft'),
  finalized('finalized'),
  granted('granted'),
  denied('denied'),
  withdrawn('withdrawn');

  const ClaimStatus(this.wire);
  @override
  final String wire;

  static ClaimStatus fromWire(String w) =>
      values.firstWhere((e) => e.wire == w, orElse: () => draft);
}

enum CoverageTag implements _WireEnum {
  exact('exact'),
  approximate('approximate'),
  boundary('boundary'),
  unknown('unknown');

  const CoverageTag(this.wire);
  @override
  final String wire;

  static CoverageTag fromWire(String w) =>
      values.firstWhere((e) => e.wire == w, orElse: () => unknown);
}

/// SourceTag — answer provenance (data-model enums.rs::SourceTag).
/// Wire values are `online` (NOT `web`) and `inferred` (NOT `infer`).
enum SourceTag implements _WireEnum {
  rule('rule'),
  kb('kb'),
  online('online'),
  inferred('inferred');

  const SourceTag(this.wire);
  @override
  final String wire;

  String get labelZh => switch (this) {
        rule => '规则',
        kb => '知识库',
        online => '联网',
        inferred => '推断',
      };

  static SourceTag fromWire(String w) =>
      values.firstWhere((e) => e.wire == w, orElse: () => inferred);
}

enum AuditCategory implements _WireEnum {
  aiSuggestion('ai_suggestion'),
  userReview('user_review'),
  stateChange('state_change'),
  mergeDecision('merge_decision'),
  cryptoOp('crypto_op'),
  export('export'),
  sync('sync');

  const AuditCategory(this.wire);
  @override
  final String wire;

  static AuditCategory fromWire(String w) =>
      values.firstWhere((e) => e.wire == w, orElse: () => stateChange);
}

enum LawLevel implements _WireEnum {
  constitution('constitution'),
  statute('statute'),
  judicialInterpretation('judicial_interpretation'),
  administrativeRegulation('administrative_regulation'),
  departmentalRule('departmental_rule'),
  localRegulation('local_regulation');

  const LawLevel(this.wire);
  @override
  final String wire;

  static LawLevel fromWire(String w) =>
      values.firstWhere((e) => e.wire == w, orElse: () => statute);
}
