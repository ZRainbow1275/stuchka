import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// Enforces the Lucide-only icon contract (spec 01 §1.7): business AND test code must use
/// StuchkaIcons, not Material `Icons.` or Cupertino `CupertinoIcons.`. The only sanctioned
/// `Icons.`/`CupertinoIcons.` references are inside this lint test itself (which names them in its
/// pattern + skip list) and the StuchkaIcons wrapper (which uses LucideIcons only).
///
/// Both `lib/` and `test/` are scanned so a forbidden Material/Cupertino icon can never regress in
/// either tree (the simple_mode_test.dart placeholder regression is now caught here too).
void main() {
  test('lib/ and test/ contain no Material Icons.* or CupertinoIcons.* references', () async {
    final offenders = <String>[];
    final iconsPattern = RegExp(r'(?<![A-Za-z_.])(Icons|CupertinoIcons)\.');

    // This lint file legitimately names `Icons.`/`CupertinoIcons.` in its own source; skip it.
    // Match on the file name (separator-agnostic) so the skip works on Windows and POSIX alike.
    const selfFileName = 'no_forbidden_icons_test.dart';

    for (final root in const ['lib', 'test']) {
      final dir = Directory(root);
      if (!dir.existsSync()) continue;
      await for (final entity in dir.list(recursive: true)) {
        if (entity is! File || !entity.path.endsWith('.dart')) continue;
        final normalized = entity.path.replaceAll('\\', '/');
        if (normalized.endsWith('/$selfFileName') || normalized == selfFileName) {
          continue;
        }
        final content = await entity.readAsString();
        for (final line in content.split('\n')) {
          // Allow comments to mention them.
          final code = line.split('//').first;
          if (iconsPattern.hasMatch(code)) {
            offenders.add('${entity.path}: ${line.trim()}');
          }
        }
      }
    }
    expect(
      offenders,
      isEmpty,
      reason: 'Use StuchkaIcons (Lucide) only. Offenders:\n${offenders.join('\n')}',
    );
  });
}
