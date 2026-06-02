// CI Emoji scanner (spec 01 §1.8 防线 2). Scans all .dart / .ts / .md / .json files under the
// package and exits 1 on any Emoji hit. Emoji blocks: U+1F300-1FAFF, U+2600-27BF, U+1F000-1F2FF.
//
// Usage: dart run tools/lint_no_emoji.dart [rootDir]
import 'dart:io';

const List<String> scannedExtensions = ['.dart', '.ts', '.md', '.json'];

/// Returns true iff [rune] is in one of the forbidden Emoji unicode blocks.
bool isEmoji(int rune) {
  return (rune >= 0x1F300 && rune <= 0x1FAFF) ||
      (rune >= 0x2600 && rune <= 0x27BF) ||
      (rune >= 0x1F000 && rune <= 0x1F2FF);
}

/// Scan a single string; returns the first offending rune or null.
int? firstEmojiRune(String text) {
  for (final r in text.runes) {
    if (isEmoji(r)) return r;
  }
  return null;
}

Future<int> scanDir(Directory root) async {
  var hits = 0;
  await for (final entity in root.list(recursive: true, followLinks: false)) {
    if (entity is! File) continue;
    final path = entity.path.replaceAll('\\', '/');
    // Skip build / generated caches.
    if (path.contains('/.dart_tool/') ||
        path.contains('/build/') ||
        path.contains('/node_modules/') ||
        path.contains('/.git/')) {
      continue;
    }
    if (!scannedExtensions.any(path.endsWith)) continue;
    final content = await entity.readAsString();
    final rune = firstEmojiRune(content);
    if (rune != null) {
      hits++;
      stderr.writeln('EMOJI U+${rune.toRadixString(16).toUpperCase()} in $path');
    }
  }
  return hits;
}

Future<void> main(List<String> args) async {
  final root = Directory(args.isNotEmpty ? args.first : '.');
  final hits = await scanDir(root);
  if (hits > 0) {
    stderr.writeln('Emoji lint failed: $hits file(s) contain Emoji.');
    exit(1);
  }
  stdout.writeln('Emoji lint passed: no Emoji found.');
}
