/// INV-07 psychological-crisis tiers (spec 05 §5.6.1). Language-neutral: NO "诊断 / 判定".
enum CrisisLevel { none, light, mid, severe }

/// Keyword-based crisis detector. Severe > mid > light precedence (a severe phrase wins even if a
/// lighter phrase also appears). The three keyword sets are the spec §5.6.1 corpus.
class CrisisDetector {
  const CrisisDetector();

  static const List<String> lightKeywords = ['无力', '太累', '想放弃', '撑不下去'];
  static const List<String> midKeywords = ['绝望', '没意义', '算了', '不想做了'];
  static const List<String> severeKeywords = ['想死', '自残', '不想活', '了此一生'];

  CrisisLevel scan(String text) {
    if (severeKeywords.any(text.contains)) return CrisisLevel.severe;
    if (midKeywords.any(text.contains)) return CrisisLevel.mid;
    if (lightKeywords.any(text.contains)) return CrisisLevel.light;
    return CrisisLevel.none;
  }
}
