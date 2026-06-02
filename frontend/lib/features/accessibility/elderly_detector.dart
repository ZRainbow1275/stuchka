/// One observed task timing sample.
class TaskTiming {
  const TaskTiming({required this.elapsedMs});
  final int elapsedMs;
}

/// Elderly-user detection (spec 05 §5.5 / brief). Two triggers (either fires):
///   1. identity classification hits `09 退休返聘` (RetiredRehired);
///   2. operation speed over the first 3 tasks > 2x the offline reference-corpus median.
///
/// Language-neutral by contract: the detector exposes NO "老年 / 长者 / 退休" wording — only a
/// boolean trigger. The UI copy that follows ("已为您调整字号 / 操作节奏较慢") never names age.
/// The reference median is an offline, never-transmitted constant (§5.5.1 零外发).
class ElderlyDetector {
  ElderlyDetector({
    int referenceMedianMs = _defaultReferenceMedianMs,
    void Function()? onDetected,
  })  : _referenceMedianMs = referenceMedianMs,
        _onDetected = onDetected;

  /// Offline reference-corpus median per-task duration (ms). R1 Beta anonymous local sample;
  /// kept as a constant here, never fetched.
  static const int _defaultReferenceMedianMs = 45000;

  final int _referenceMedianMs;
  final void Function()? _onDetected;
  final List<TaskTiming> _samples = [];
  bool _detected = false;

  bool get detected => _detected;

  /// Trigger 1: identity-based. RetiredRehired immediately marks the user.
  void observeIdentityIsRetiredRehired(bool isRetiredRehired) {
    if (isRetiredRehired) _markDetected();
  }

  /// Trigger 2: timing-based. Collects the first 3 tasks; if the mean elapsed > 2x the reference
  /// median, the user is marked. Returns true on the call that flips detection on.
  bool observeTask(TaskTiming t) {
    if (_detected) return false;
    _samples.add(t);
    if (_samples.length < 3) return false;
    final mine = _samples.map((s) => s.elapsedMs).reduce((a, b) => a + b) / _samples.length;
    if (mine > _referenceMedianMs * 2) {
      _markDetected();
      return true;
    }
    return false;
  }

  void _markDetected() {
    if (_detected) return;
    _detected = true;
    _onDetected?.call();
  }
}
