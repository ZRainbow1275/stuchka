/// Public, no-affiliation助援热线 (spec 05 §5.6.3 / §5.7). Listing these is public information and
/// carries the explicit "与本系统无任何合作关系" disclaimer at every render site.
class Hotline {
  const Hotline(this.code, this.desc);
  final String code;
  final String desc;
}

const List<Hotline> kCrisisHotlines = [
  Hotline('12320', '国家卫生健康委心理援助'),
  Hotline('12351', '全国总工会职工服务'),
  Hotline('12348', '司法部法律援助'),
];

/// Legal-aid / union hotlines surfaced inside the INV-10 high-risk gate panel.
const List<Hotline> kLegalAidHotlines = [
  Hotline('12348', '司法部法律援助热线'),
  Hotline('12351', '全国总工会职工维权'),
];

const String kHotlineDisclaimer = '本系统列出热线仅为公开信息，与本系统无任何合作关系。';
