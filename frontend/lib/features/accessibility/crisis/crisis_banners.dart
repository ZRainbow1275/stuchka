import 'package:flutter/material.dart';

import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';
import '../hotlines.dart';

/// INV-07 Level-1 empathy banner — inline, non-blocking (spec 05 §5.6.2). Language-neutral copy.
class CrisisLevel1Banner extends StatelessWidget {
  const CrisisLevel1Banner({super.key, this.onDismiss});

  final VoidCallback? onDismiss;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    return Material(
      color: Theme.of(context).colorScheme.surface,
      child: Container(
        decoration: BoxDecoration(
          border: Border(left: BorderSide(color: colors.inkBlue, width: 4)),
        ),
        padding: const EdgeInsets.all(12),
        child: Row(
          children: [
            Icon(StuchkaIcons.heart, size: 18, color: colors.inkBlue),
            const SizedBox(width: 8),
            const Expanded(
              child: Text('维权是漫长的过程，您已经走到这里。我们继续陪您下一步。'),
            ),
            TextButton(onPressed: onDismiss, child: const Text('知道了')),
          ],
        ),
      ),
    );
  }
}

/// INV-07 Level-2 soft reminder dialog. The DEFAULT button (a FilledButton) is "继续操作" — never
/// the reverse (spec 05 §5.6.3). Returns 'continue' | 'help' | null.
class CrisisLevel2Dialog {
  static Future<String?> show(BuildContext ctx) {
    return showDialog<String>(
      context: ctx,
      builder: (dctx) => AlertDialog(
        title: const Text('检测到您可能需要支持'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text('如您需要，可拨打以下公开热线：'),
            const SizedBox(height: 8),
            ...kCrisisHotlines.map((h) => _HotlineTile(hotline: h)),
            const SizedBox(height: 12),
            Text(
              kHotlineDisclaimer,
              style: Theme.of(dctx).textTheme.bodySmall,
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(dctx, 'help'),
            child: const Text('打开求助页面'),
          ),
          // DEFAULT = 继续操作 (FilledButton, autofocus).
          FilledButton(
            autofocus: true,
            onPressed: () => Navigator.pop(dctx, 'continue'),
            child: const Text('我没事，继续操作'),
          ),
        ],
      ),
    );
  }
}

class _HotlineTile extends StatelessWidget {
  const _HotlineTile({required this.hotline});
  final Hotline hotline;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        children: [
          Icon(StuchkaIcons.phone, size: 16, color: colors.inkBlue),
          const SizedBox(width: 8),
          Text(hotline.code, style: const TextStyle(fontWeight: FontWeight.w700)),
          const SizedBox(width: 8),
          Expanded(child: Text(hotline.desc)),
        ],
      ),
    );
  }
}
