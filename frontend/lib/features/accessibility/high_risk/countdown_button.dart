import 'dart:async';

import 'package:flutter/material.dart';

/// Timings captured across a full INV-10 two-stage confirm flow (compliance/05 §2.2 step5 / §5.1 /
/// §9.1). Reported to the host gate when the user finally commits so the audit four-tuple
/// (compliance/05 §2.3) can record `cooldown_actual_ms` and the §9.1 assertions can be checked.
class ConfirmTimings {
  const ConfirmTimings({
    required this.requiresSecondPress,
    required this.firstCountdownMs,
    required this.secondPressGapMs,
    required this.totalFlowMs,
  });

  /// Always true for the INV-10 gate (compliance/05 §9.1 `requires_second_press`).
  final bool requiresSecondPress;

  /// Time the first (read) countdown actually ran (>= 8000ms; compliance/05 §9.1 `countdown_seconds == 8`).
  final int firstCountdownMs;

  /// Gap between the first press (revealing 再次确认) and the final commit (>= 3000ms; §9.1
  /// `second_press_gap_ms >= 3000`).
  final int secondPressGapMs;

  /// Total elapsed from open to final commit (>= 11000ms; §9.1 `total_flow_ms >= 11000`).
  final int totalFlowMs;
}

/// INV-10 two-stage forced-cooldown confirm button (compliance/05 §2.2 step5 / §5.1 / spec 05 §5.7.2).
///
/// Engineering invariants (asserted by inv10_gate_test):
///   - Stage 1 — an 8-second read countdown; the primary button is DISABLED before it reaches 0 and
///     stays disabled until [enabled] is true (the "我已完整阅读" checkbox must be ticked).
///   - First press does NOT confirm — it reveals a "再次确认" bar (compliance/05 §2.2 step5).
///   - Stage 2 — a further 3-second gate; the final commit is disabled until the 3s elapses.
///   - Total flow is therefore >= 8 + 3 = 11 seconds (compliance/05 §5.1 "总流程时长 至少 8 + 3 = 11 秒").
///   - [onPressed] fires only after the second press, and receives the measured [ConfirmTimings].
///
/// Elapsed time is accumulated from the 1-second periodic ticks (NOT a wall-clock Stopwatch) so the
/// measured timings advance correctly under the widget-test fake clock.
class CountdownButton extends StatefulWidget {
  const CountdownButton({
    super.key,
    required this.seconds,
    required this.enabled,
    required this.label,
    required this.onPressed,
    this.secondPressGapSeconds = 3,
  });

  /// Stage-1 read-countdown length (== 8 for INV-10).
  final int seconds;

  /// Whether the "我已完整阅读" checkbox is ticked (gates stage-1 completion).
  final bool enabled;

  /// Primary button label.
  final String label;

  /// Stage-2 gate length before the final commit enables (== 3 for INV-10).
  final int secondPressGapSeconds;

  /// Fired once, only after the second press completes. Carries the measured flow timings.
  final void Function(ConfirmTimings timings) onPressed;

  @override
  State<CountdownButton> createState() => _CountdownButtonState();
}

class _CountdownButtonState extends State<CountdownButton> {
  late int _left = widget.seconds;
  int _gapLeft = 0;
  bool _secondStage = false;
  Timer? _t;
  Timer? _gapT;

  /// Elapsed milliseconds accumulated from the 1s ticks (test-clock safe).
  int _elapsedMs = 0;
  int _firstCountdownMs = 0;
  int _firstPressAtMs = 0;

  @override
  void initState() {
    super.initState();
    _t = Timer.periodic(const Duration(seconds: 1), (_) {
      if (!mounted) return;
      _elapsedMs += 1000;
      if (_left <= 0) {
        _t?.cancel();
        return;
      }
      setState(() => _left -= 1);
      if (_left == 0) _firstCountdownMs = _elapsedMs;
    });
  }

  @override
  void dispose() {
    _t?.cancel();
    _gapT?.cancel();
    super.dispose();
  }

  void _onFirstPress() {
    if (_secondStage) return;
    if (_firstCountdownMs == 0) _firstCountdownMs = _elapsedMs;
    _firstPressAtMs = _elapsedMs;
    _gapLeft = widget.secondPressGapSeconds;
    setState(() => _secondStage = true);
    _gapT = Timer.periodic(const Duration(seconds: 1), (_) {
      if (!mounted) return;
      _elapsedMs += 1000;
      if (_gapLeft <= 0) {
        _gapT?.cancel();
        return;
      }
      setState(() => _gapLeft -= 1);
    });
  }

  void _onCommit() {
    widget.onPressed(ConfirmTimings(
      requiresSecondPress: true,
      firstCountdownMs: _firstCountdownMs,
      secondPressGapMs: _elapsedMs - _firstPressAtMs,
      totalFlowMs: _elapsedMs,
    ));
  }

  @override
  Widget build(BuildContext context) {
    final error = Theme.of(context).colorScheme.error;
    final stageOneReady = widget.enabled && _left == 0;

    if (!_secondStage) {
      // Stage 1: read countdown. First press reveals the second-confirm bar.
      return FilledButton(
        onPressed: stageOneReady ? _onFirstPress : null,
        style: FilledButton.styleFrom(
          backgroundColor: error,
          disabledBackgroundColor: error.withValues(alpha: 0.45),
        ),
        child: Text(_left == 0 ? widget.label : '${widget.label}（${_left}s）'),
      );
    }

    // Stage 2: the 再次确认 bar with its own 3s gate.
    final commitReady = _gapLeft == 0;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        const Text('再次确认：'),
        const SizedBox(width: 8),
        FilledButton(
          key: const Key('inv10_second_confirm'),
          onPressed: commitReady ? _onCommit : null,
          style: FilledButton.styleFrom(
            backgroundColor: error,
            disabledBackgroundColor: error.withValues(alpha: 0.45),
          ),
          child: Text(commitReady ? '确认提交' : '确认提交（${_gapLeft}s）'),
        ),
      ],
    );
  }
}
