// Font fetcher (spec 01 §1.4). Downloads the redistributable SIL OFL fonts into assets/fonts/ and
// toggles the managed `fonts:` block in pubspec.yaml on (so the build picks them up). All three
// families are OFL-licensed and redistributable:
//
//   - Source Han Sans SC      (Adobe / Google "Noto Sans CJK SC" share the same OFL source)
//   - Source Han Serif SC     (Adobe / Google "Noto Serif CJK SC")
//   - JetBrains Mono          (OFL, JetBrains)
//
// Why this is a *script* and not committed binaries: the OTFs are tens of MB each. Committing them
// would bloat the repo and is unnecessary — the build stays green WITHOUT the OTFs because
// StuchkaTheme.fontFamily falls back to the system font when the family is absent (graceful
// fallback). Run this script when you want the bundled GB-45438 公文 typefaces.
//
// Usage:
//   dart run tools/fetch_fonts.dart            # download (skips files already present) + enable block
//   dart run tools/fetch_fonts.dart --check    # report which fonts are present; no download
//   dart run tools/fetch_fonts.dart --disable  # comment the fonts: block back out (revert)
//
// Zero runtime network: this script is a *build-time* tool only. The app never fetches fonts at
// runtime (prd §5.5.1 零外发).
import 'dart:io';

/// One downloadable font asset: the on-disk filename + a list of mirror URLs (tried in order).
class _FontAsset {
  const _FontAsset(this.fileName, this.urls);
  final String fileName;
  final List<String> urls;
}

// Official OFL release sources. Google Fonts' googlefonts-cn mirror redistributes the Adobe Source
// Han / Noto CJK OFL builds; JetBrains Mono ships from the JetBrains repo. All redistributable.
const List<_FontAsset> _fonts = [
  _FontAsset('SourceHanSansSC-Regular.otf', [
    'https://github.com/notofonts/noto-cjk/raw/main/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf',
  ]),
  _FontAsset('SourceHanSansSC-Medium.otf', [
    'https://github.com/notofonts/noto-cjk/raw/main/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Medium.otf',
  ]),
  _FontAsset('SourceHanSansSC-Bold.otf', [
    'https://github.com/notofonts/noto-cjk/raw/main/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Bold.otf',
  ]),
  _FontAsset('SourceHanSerifSC-Regular.otf', [
    'https://github.com/notofonts/noto-cjk/raw/main/Serif/OTF/SimplifiedChinese/NotoSerifCJKsc-Regular.otf',
  ]),
  _FontAsset('SourceHanSerifSC-Bold.otf', [
    'https://github.com/notofonts/noto-cjk/raw/main/Serif/OTF/SimplifiedChinese/NotoSerifCJKsc-Bold.otf',
  ]),
  _FontAsset('JetBrainsMono-Regular.ttf', [
    'https://github.com/JetBrains/JetBrainsMono/raw/master/fonts/ttf/JetBrainsMono-Regular.ttf',
  ]),
];

const _beginMarker = '# --- BEGIN STUCHKA FONTS (managed by tools/fetch_fonts.dart) ---';
const _endMarker = '# --- END STUCHKA FONTS ---';

Future<void> main(List<String> args) async {
  final check = args.contains('--check');
  final disable = args.contains('--disable');

  final fontsDir = Directory('assets/fonts');
  if (!fontsDir.existsSync()) fontsDir.createSync(recursive: true);

  if (disable) {
    _toggleFontsBlock(enable: false);
    stdout.writeln('Disabled (commented) the fonts: block in pubspec.yaml. '
        'Build falls back to system fonts.');
    return;
  }

  if (check) {
    var allPresent = true;
    for (final f in _fonts) {
      final present = File('assets/fonts/${f.fileName}').existsSync();
      stdout.writeln('${present ? "[present]" : "[missing]"} ${f.fileName}');
      allPresent = allPresent && present;
    }
    stdout.writeln(allPresent
        ? 'All fonts present. Run without --check to (re)enable the pubspec fonts: block.'
        : 'Some fonts missing. Run `dart run tools/fetch_fonts.dart` to download.');
    return;
  }

  final client = HttpClient()..connectionTimeout = const Duration(seconds: 30);
  var downloaded = 0;
  var present = 0;
  var failed = 0;
  try {
    for (final f in _fonts) {
      final out = File('assets/fonts/${f.fileName}');
      if (out.existsSync() && out.lengthSync() > 0) {
        present++;
        stdout.writeln('[skip]  ${f.fileName} (already present)');
        continue;
      }
      final ok = await _download(client, f, out);
      if (ok) {
        downloaded++;
      } else {
        failed++;
      }
    }
  } finally {
    client.close(force: true);
  }

  stdout.writeln('Fonts: $downloaded downloaded, $present already present, $failed failed.');

  // Only enable the pubspec block if every declared asset now exists — otherwise the build would
  // fail on a missing font asset. Partial downloads leave the block commented (graceful fallback).
  final allPresent =
      _fonts.every((f) => File('assets/fonts/${f.fileName}').existsSync());
  if (allPresent) {
    _toggleFontsBlock(enable: true);
    stdout.writeln('Enabled the fonts: block in pubspec.yaml. '
        'Run `flutter pub get` then rebuild.');
  } else {
    stdout.writeln('Not all fonts present — left the pubspec fonts: block commented so the build '
        'stays green with the system-font fallback. Re-run when the network is available.');
    if (failed > 0) exitCode = 1;
  }
}

Future<bool> _download(HttpClient client, _FontAsset f, File out) async {
  for (final url in f.urls) {
    try {
      stdout.writeln('[get]   ${f.fileName} <- $url');
      final req = await client.getUrl(Uri.parse(url));
      final res = await req.close();
      if (res.statusCode != 200) {
        stdout.writeln('        HTTP ${res.statusCode}; trying next mirror');
        continue;
      }
      final sink = out.openWrite();
      await res.pipe(sink);
      await sink.flush();
      await sink.close();
      if (out.existsSync() && out.lengthSync() > 0) return true;
    } catch (e) {
      stdout.writeln('        error: $e; trying next mirror');
    }
  }
  stdout.writeln('[fail]  ${f.fileName}: all mirrors failed');
  return false;
}

/// Toggle the managed fonts: block between the BEGIN/END markers in pubspec.yaml. `enable` true
/// strips the leading `# ` from every non-marker line in the block; false re-adds it. Idempotent.
void _toggleFontsBlock({required bool enable}) {
  final pubspec = File('pubspec.yaml');
  final lines = pubspec.readAsLinesSync();
  final beginIdx = lines.indexWhere((l) => l.contains(_beginMarker));
  final endIdx = lines.indexWhere((l) => l.contains(_endMarker));
  if (beginIdx < 0 || endIdx < 0 || endIdx <= beginIdx) {
    stderr.writeln('Could not find the managed fonts: markers in pubspec.yaml; skipping toggle.');
    return;
  }
  for (var i = beginIdx + 1; i < endIdx; i++) {
    final line = lines[i];
    if (enable) {
      // Strip exactly one leading "  # " / "  #" indentation-preserving comment.
      lines[i] = line.replaceFirst(RegExp(r'^(\s*)#\s?'), r'$1');
    } else {
      if (!RegExp(r'^\s*#').hasMatch(line) && line.trim().isNotEmpty) {
        final indent = RegExp(r'^(\s*)').firstMatch(line)!.group(1)!;
        lines[i] = '$indent# ${line.substring(indent.length)}';
      }
    }
  }
  pubspec.writeAsStringSync('${lines.join('\n')}\n');
}
