import 'crisis_detector.dart';

/// One per-turn emotional signal sample (the [CrisisDetector] result for a user message).
class EmotionSample {
  const EmotionSample(this.at, this.level);
  final DateTime at;
  final CrisisLevel level;
}

/// M11 历史会话情绪 (R1.5; prd/04 §4 M11). An in-memory, session-scoped record of the conversation's
/// per-turn crisis-level signals, used to make the INV-07 supportive response **history-aware**: a
/// SUSTAINED pattern of concern across recent turns keeps the gentle Level-1 support present even
/// when the latest single turn is neutral, instead of the support vanishing the moment one message
/// reads as calm.
///
/// Deliberate boundaries (genuineness + do-no-harm):
///   * NEVER persisted — the emotional trajectory lives only for the session and is dropped on exit
///     (privacy; an emotional history is not written to disk).
///   * Behavioral support only — this drives *more* gentle support, not surveillance. A user-facing
///     emotion-curve VISUALIZATION and MULTIMODAL (语音/表情) input remain documented R1.5+ seams
///     pending UX/ethics review; this layer fabricates nothing and shows the user no clinical chart.
class EmotionTrend {
  EmotionTrend({this.window = 5, this.sustainThreshold = 3});

  /// How many most-recent samples define "recent".
  final int window;

  /// How many concerning samples within the window count as a sustained pattern.
  final int sustainThreshold;

  final List<EmotionSample> _samples = [];

  /// All recorded samples in order (read-only view).
  List<EmotionSample> get samples => List.unmodifiable(_samples);

  /// Record one scanned turn. `at` is supplied by the caller (testable; the panel passes the
  /// real wall-clock time).
  void record(CrisisLevel level, DateTime at) {
    _samples.add(EmotionSample(at, level));
  }

  List<EmotionSample> get _recent =>
      _samples.length <= window ? _samples : _samples.sublist(_samples.length - window);

  /// True when the recent window holds at least [sustainThreshold] non-`none` signals — i.e. a
  /// sustained pattern of concern, so support should persist across turns.
  bool get sustainedConcern =>
      _recent.where((s) => s.level != CrisisLevel.none).length >= sustainThreshold;

  /// The most severe level seen in the recent window (`none` when the window is all-calm/empty).
  /// Lets the caller choose the gentlest adequate support level for a sustained pattern.
  CrisisLevel get recentPeak {
    CrisisLevel peak = CrisisLevel.none;
    for (final s in _recent) {
      if (s.level.index > peak.index) peak = s.level;
    }
    return peak;
  }

  /// Drop all samples (e.g. on leaving the case session).
  void clear() => _samples.clear();
}
