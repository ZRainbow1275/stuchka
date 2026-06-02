import 'high_risk/disclaimer_payload.dart';

/// Public, no-affiliation 法援 / 工会 / 心理援助资源 (compliance/05 §6 / spec 05 §5.6.3 / §5.7). Listing
/// these is public information and carries the explicit "与本系统无任何合作关系" disclaimer at every render
/// site. One shared list (compliance/05 §6: "由 ... 统一维护；6 场景模板与 INV-07 都从此处读取，避免不一致").
class Hotline {
  const Hotline(this.code, this.desc);

  /// Phone number OR web url (compliance/05 §6 lists both 号码 and 链接).
  final String code;
  final String desc;
}

/// The single source of truth for every legal-aid / union / crisis / web resource (compliance/05
/// §6 table). Scenario-scoped subsets are derived from this list — never hand-maintained twice.
class LegalAidResources {
  LegalAidResources._();

  static const Hotline legalAid = Hotline('12348', '全国法律援助热线（所有场景）');
  static const Hotline union = Hotline('12351', '全国总工会维权热线（所有场景，含心理咨询通道）');
  static const Hotline crisis = Hotline('12320', '全国心理援助热线（INV-07 / 工伤）');
  static const Hotline laborInspection = Hotline('12333', '全国劳动保障监察（拒不支付劳动报酬罪 / 工伤）');
  static const Hotline womenRights = Hotline('12338', '全国妇联维权热线（性骚扰 / 三期）');
  static const Hotline minorProtection = Hotline('12345', '未成年人保护 / 当地未保办（未成年劳动者）');
  static const Hotline aclaWeb = Hotline('https://www.acla.org.cn/', '中华全国律师协会公益法援平台');
  static const Hotline legalServiceWeb = Hotline('https://www.12348.gov.cn/', '中国法律服务网');

  /// Every resource (used by the §6 completeness QA scan).
  static const List<Hotline> all = [
    legalAid,
    union,
    crisis,
    laborInspection,
    womenRights,
    minorProtection,
    aclaWeb,
    legalServiceWeb,
  ];

  /// Resources surfaced inside an INV-10 high-risk gate, scoped per scenario (compliance/05 §6
  /// 适用 column). Every scenario gets 法援 + 工会 + 两个公益法援网站; sensitive scenarios add their
  /// scoped hotlines (劳动监察 / 妇联 / 未成年保护 / 心理援助).
  static List<Hotline> forScenario(HighRiskScenario scenario) {
    final base = <Hotline>[legalAid, union];
    switch (scenario) {
      case HighRiskScenario.criminalReportExport:
        base.add(laborInspection);
        break;
      case HighRiskScenario.protectedScenario:
        base
          ..add(womenRights)
          ..add(minorProtection)
          ..add(laborInspection)
          ..add(crisis);
        break;
      case HighRiskScenario.resignAdvice:
      case HighRiskScenario.settlementBelow80:
      case HighRiskScenario.abandonClaim:
      case HighRiskScenario.groupRepresentativeAuth:
      case HighRiskScenario.groupCreation:
        break;
    }
    base
      ..add(aclaWeb)
      ..add(legalServiceWeb);
    return base;
  }
}

/// INV-07 Level-2 / Level-3 crisis hotlines (compliance/05 §4.2 / §6). 心理援助优先。
const List<Hotline> kCrisisHotlines = [
  LegalAidResources.crisis,
  LegalAidResources.union,
  LegalAidResources.legalAid,
];

/// Legacy alias kept for the INV-10 gate panel default (now derived per-scenario via
/// [LegalAidResources.forScenario]). Retained so older imports keep compiling.
const List<Hotline> kLegalAidHotlines = [
  LegalAidResources.legalAid,
  LegalAidResources.union,
];

const String kHotlineDisclaimer = '本系统列出的热线仅为公开信息，与本系统无任何合作关系。';
