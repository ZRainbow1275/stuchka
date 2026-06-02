import 'dart:async';

import 'package:flutter/material.dart';

/// INV-10 8-second forced-cooldown confirm button (spec 05 §5.7.2).
///
/// Engineering invariants (asserted by inv10_gate_test):
///   - the countdown starts at [seconds] (== 8 at open) and is DISABLED before it reaches 0;
///   - even after the countdown reaches 0 the button stays disabled until [enabled] is true
///     (the "我已完整阅读" checkbox must be ticked);
///   - the label shows the remaining seconds while counting down.
class CountdownButton extends StatefulWidget {
  const CountdownButton({
    super.key,
    required this.seconds,
    required this.enabled,
    required this.label,
    required this.onPressed,
  });

  final int seconds;
  final bool enabled;
  final String label;
  final VoidCallback onPressed;

  @override
  State<CountdownButton> createState() => _CountdownButtonState();
}

class _CountdownButtonState extends State<CountdownButton> {
  late int _left = widget.seconds;
  Timer? _t;

  @override
  void initState() {
    super.initState();
    _t = Timer.periodic(const Duration(seconds: 1), (_) {
      if (!mounted) return;
      if (_left <= 0) {
        _t?.cancel();
        return;
      }
      setState(() => _left -= 1);
    });
  }

  @override
  void dispose() {
    _t?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final ready = widget.enabled && _left == 0;
    return FilledButton(
      onPressed: ready ? widget.onPressed : null,
      style: FilledButton.styleFrom(
        backgroundColor: Theme.of(context).colorScheme.error,
        disabledBackgroundColor:
            Theme.of(context).colorScheme.error.withValues(alpha: 0.45),
      ),
      child: Text(_left == 0 ? widget.label : '${widget.label}（${_left}s）'),
    );
  }
}
