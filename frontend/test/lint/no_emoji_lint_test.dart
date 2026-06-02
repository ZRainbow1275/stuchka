import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

// Pull in the scanner logic directly (tools/ is outside lib/, imported by relative path).
import '../../tools/lint_no_emoji.dart' as scanner;

void main() {
  group('no-emoji lint (spec 01 §1.8)', () {
    test('isEmoji classifies the forbidden unicode blocks', () {
      expect(scanner.isEmoji(0x1F600), isTrue); // grinning face block
      expect(scanner.isEmoji(0x2600), isTrue); // misc symbols
      expect(scanner.isEmoji(0x1F004), isTrue); // mahjong (1F000 block)
      expect(scanner.isEmoji('A'.runes.first), isFalse);
      expect(scanner.isEmoji('劳'.runes.first), isFalse); // CJK is allowed
    });

    test('firstEmojiRune finds an emoji in mixed text', () {
      expect(scanner.firstEmojiRune('hello world'), isNull);
      expect(scanner.firstEmojiRune('劳动纠纷'), isNull);
      expect(scanner.firstEmojiRune('done \u{1F44D}'), isNotNull);
    });

    test('the lib/ + test/ + tools/ tree contains no Emoji', () async {
      for (final dir in ['lib', 'test', 'tools']) {
        final hits = await scanner.scanDir(Directory(dir));
        expect(hits, 0, reason: '$dir contains Emoji (spec 01 §1.8 forbids it)');
      }
    });
  });
}
