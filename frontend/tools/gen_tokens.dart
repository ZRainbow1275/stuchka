// Token generator (spec 01 §1.1). Reads assets/tokens/tokens.base.json (+ light/dark overrides)
// and regenerates lib/theme/generated/tokens.g.dart and web/editor/src/tokens.css.
//
// The generated Dart file is the single source of color/type/spacing constants for business code
// (no hardcoded Color literals). This tool is the only writer of tokens.g.dart.
//
// Usage: dart run tools/gen_tokens.dart
import 'dart:convert';
import 'dart:io';

String _hexToArgb(String hex) {
  final h = hex.replaceAll('#', '');
  return '0xFF${h.toUpperCase()}';
}

void main() {
  final base = jsonDecode(File('assets/tokens/tokens.base.json').readAsStringSync())
      as Map<String, dynamic>;
  final dark = jsonDecode(File('assets/tokens/tokens.dark.json').readAsStringSync())
      as Map<String, dynamic>;
  // tokens.print.json (spec 01 §1.1 / §1.10) — 公文打印 token，平台无关，无深色覆盖。
  final print = jsonDecode(File('assets/tokens/tokens.print.json').readAsStringSync())
      as Map<String, dynamic>;

  final brand = base['color']['brand'] as Map<String, dynamic>;
  final neutral = base['color']['neutral'] as Map<String, dynamic>;
  final semantic = base['color']['semantic'] as Map<String, dynamic>;
  final source = base['color']['source'] as Map<String, dynamic>;
  final darkNeutral = dark['color']['neutral'] as Map<String, dynamic>;
  final darkBrand = dark['color']['brand'] as Map<String, dynamic>;
  final type = base['typography'] as Map<String, dynamic>;
  final spacing = base['spacing'] as Map<String, dynamic>;
  final printText = print['color']['text'] as Map<String, dynamic>;
  final printPage = print['color']['page'] as Map<String, dynamic>;
  final printType = print['typography'] as Map<String, dynamic>;
  final printSize = printType['size'] as Map<String, dynamic>;
  final printPageGeom = print['page'] as Map<String, dynamic>;
  final printGb = print['gb45438'] as Map<String, dynamic>;

  final buf = StringBuffer()
    ..writeln('// GENERATED FILE — do not edit by hand.')
    ..writeln('// Regenerate via: dart run tools/gen_tokens.dart')
    ..writeln('// ignore_for_file: public_member_api_docs')
    ..writeln("import 'dart:ui';")
    ..writeln()
    ..writeln('class StuchkaTokens {')
    ..writeln('  StuchkaTokens._();')
    ..writeln('  static const Color sealRed = Color(${_hexToArgb(brand['seal_red'] as String)});')
    ..writeln('  static const Color sealRedDeep = Color(${_hexToArgb(brand['seal_red_deep'] as String)});')
    ..writeln('  static const Color sealRedSoft = Color(${_hexToArgb(brand['seal_red_soft'] as String)});')
    ..writeln('  static const Color inkBlue = Color(${_hexToArgb(brand['ink_blue'] as String)});')
    ..writeln('  static const Color inkBlueDeep = Color(${_hexToArgb(brand['ink_blue_deep'] as String)});')
    ..writeln('  static const Color inkBlueSoft = Color(${_hexToArgb(brand['ink_blue_soft'] as String)});')
    ..writeln('  static const Color paper = Color(${_hexToArgb(neutral['paper'] as String)});')
    ..writeln('  static const Color rice = Color(${_hexToArgb(neutral['rice'] as String)});')
    ..writeln('  static const Color stone = Color(${_hexToArgb(neutral['stone'] as String)});')
    ..writeln('  static const Color ash = Color(${_hexToArgb(neutral['ash'] as String)});')
    ..writeln('  static const Color char = Color(${_hexToArgb(neutral['char'] as String)});')
    ..writeln('  static const Color riskHigh = Color(${_hexToArgb(semantic['risk_high'] as String)});')
    ..writeln('  static const Color riskMid = Color(${_hexToArgb(semantic['risk_mid'] as String)});')
    ..writeln('  static const Color riskLow = Color(${_hexToArgb(semantic['risk_low'] as String)});')
    ..writeln('  static const Color abstain = Color(${_hexToArgb(semantic['abstain'] as String)});')
    ..writeln('  static const Color frozen = Color(${_hexToArgb(semantic['frozen'] as String)});')
    ..writeln('  static const Color sourceRule = Color(${_hexToArgb(source['rule'] as String)});')
    ..writeln('  static const Color sourceKb = Color(${_hexToArgb(source['kb'] as String)});')
    ..writeln('  static const Color sourceOnline = Color(${_hexToArgb(source['online'] as String)});')
    ..writeln('  static const Color sourceInferred = Color(${_hexToArgb(source['inferred'] as String)});')
    ..writeln('  static const Color paperDark = Color(${_hexToArgb(darkNeutral['paper'] as String)});')
    ..writeln('  static const Color riceDark = Color(${_hexToArgb(darkNeutral['rice'] as String)});')
    ..writeln('  static const Color stoneDark = Color(${_hexToArgb(darkNeutral['stone'] as String)});')
    ..writeln('  static const Color ashDark = Color(${_hexToArgb(darkNeutral['ash'] as String)});')
    ..writeln('  static const Color charDark = Color(${_hexToArgb(darkNeutral['char'] as String)});')
    ..writeln('  static const Color sealRedDark = Color(${_hexToArgb(darkBrand['seal_red'] as String)});')
    ..writeln('  static const Color inkBlueDark = Color(${_hexToArgb(darkBrand['ink_blue'] as String)});')
    ..writeln('}')
    ..writeln()
    ..writeln('class StuchkaTypography {')
    ..writeln('  StuchkaTypography._();')
    ..writeln('  static const double display = ${type['display']};')
    ..writeln('  static const double title = ${type['title']};')
    ..writeln('  static const double heading = ${type['heading']};')
    ..writeln('  static const double body = ${type['body']};')
    ..writeln('  static const double caption = ${type['caption']};')
    ..writeln('  static const double mono = ${type['mono']};')
    ..writeln('}')
    ..writeln()
    ..writeln('class StuchkaSpacingTokens {')
    ..writeln('  StuchkaSpacingTokens._();')
    ..writeln('  static const double xs = ${spacing['xs']};')
    ..writeln('  static const double sm = ${spacing['sm']};')
    ..writeln('  static const double md = ${spacing['md']};')
    ..writeln('  static const double lg = ${spacing['lg']};')
    ..writeln('  static const double xl = ${spacing['xl']};')
    ..writeln('  static const double xxl = ${spacing['xxl']};')
    ..writeln('  static const double radiusSeal = ${spacing['radius_seal']};')
    ..writeln('  static const double radiusCard = ${spacing['radius_card']};')
    ..writeln('  static const double radiusDialog = ${spacing['radius_dialog']};')
    ..writeln('}')
    ..writeln()
    ..writeln('/// GB 45438-2025 公文打印 token (spec 01 §1.1 / §1.10). Print is platform-independent and')
    ..writeln('/// has no dark override; the final PDF + four-layer watermark are owned by the Rust')
    ..writeln('/// crates/document (宪法 D5) — these constants drive the editor-side preview only.')
    ..writeln('class StuchkaPrintTokens {')
    ..writeln('  StuchkaPrintTokens._();')
    ..writeln('  // text colors')
    ..writeln('  static const Color textBody = Color(${_hexToArgb(printText['body'] as String)});')
    ..writeln('  static const Color textHeading = Color(${_hexToArgb(printText['heading'] as String)});')
    ..writeln('  static const Color textMuted = Color(${_hexToArgb(printText['muted'] as String)});')
    ..writeln('  static const Color textSeal = Color(${_hexToArgb(printText['seal'] as String)});')
    ..writeln('  static const Color ruleLine = Color(${_hexToArgb(printText['rule_line'] as String)});')
    ..writeln('  // page colors')
    ..writeln('  static const Color pageBackground = Color(${_hexToArgb(printPage['background'] as String)});')
    ..writeln('  static const Color headerBand = Color(${_hexToArgb(printPage['header_band'] as String)});')
    ..writeln('  static const Color watermarkHint = Color(${_hexToArgb(printPage['watermark_hint'] as String)});')
    ..writeln('  // typography')
    ..writeln("  static const String fontFamilyBody = '${printType['fontFamilyBody']}';")
    ..writeln("  static const String fontFamilyHeading = '${printType['fontFamilyHeading']}';")
    ..writeln("  static const String fontFamilyMono = '${printType['fontFamilyMono']}';")
    ..writeln("  static const String fontFamilySeal = '${printType['fontFamilySeal']}';")
    ..writeln('  static const double sizeTitle = ${printSize['title']};')
    ..writeln('  static const double sizeHeading1 = ${printSize['heading1']};')
    ..writeln('  static const double sizeHeading2 = ${printSize['heading2']};')
    ..writeln('  static const double sizeBody = ${printSize['body']};')
    ..writeln('  static const double sizeFootnote = ${printSize['footnote']};')
    ..writeln('  static const double sizeHeaderLabel = ${printSize['header_label']};')
    ..writeln('  static const double lineHeight = ${printType['lineHeight']};')
    ..writeln('  static const double firstLineIndentEm = ${printType['firstLineIndentEm']};')
    ..writeln('  // page geometry (mm)')
    ..writeln('  static const double pageWidthMm = ${printPageGeom['widthMm']};')
    ..writeln('  static const double pageHeightMm = ${printPageGeom['heightMm']};')
    ..writeln('  static const double marginTopMm = ${printPageGeom['marginTopMm']};')
    ..writeln('  static const double marginBottomMm = ${printPageGeom['marginBottomMm']};')
    ..writeln('  static const double marginLeftMm = ${printPageGeom['marginLeftMm']};')
    ..writeln('  static const double marginRightMm = ${printPageGeom['marginRightMm']};')
    ..writeln('  static const double headerHeightMm = ${printPageGeom['headerHeightMm']};')
    ..writeln('  static const double footerHeightMm = ${printPageGeom['footerHeightMm']};')
    ..writeln('  // GB 45438 四层标识打印参数 (compliance/01 唯一权威)')
    ..writeln("  static const String gbHeaderLabelText = '${printGb['headerLabelText']}';")
    ..writeln("  static const String gbHeaderLabelAlign = '${printGb['headerLabelAlign']}';")
    ..writeln('  static const bool gbPerPageHeader = ${printGb['perPageHeader']};')
    ..writeln('  static const bool gbFirstPageDeclaration = ${printGb['firstPageDeclaration']};')
    ..writeln('  static const bool gbYieldsToIdentityHeader = ${printGb['yieldsToIdentityHeader']};')
    ..writeln('}');

  File('lib/theme/generated/tokens.g.dart').writeAsStringSync(buf.toString());

  // CSS variables for the Tiptap editor.
  final css = StringBuffer()
    ..writeln('/* GENERATED — dart run tools/gen_tokens.dart */')
    ..writeln(':root {')
    ..writeln('  --seal-red: ${brand['seal_red']};')
    ..writeln('  --ink-blue: ${brand['ink_blue']};')
    ..writeln('  --paper: ${neutral['paper']};')
    ..writeln('  --char: ${neutral['char']};')
    ..writeln('}');
  final cssDir = Directory('web/editor/src');
  cssDir.createSync(recursive: true);
  File('web/editor/src/tokens.css').writeAsStringSync(css.toString());

  stdout.writeln('Generated lib/theme/generated/tokens.g.dart + web/editor/src/tokens.css');
}
