import 'package:flutter/material.dart';

import 'generated/tokens.g.dart';

/// Palette seed for one brightness (drawn entirely from generated tokens).
class StuchkaPalette {
  const StuchkaPalette({
    required this.brightness,
    required this.inkBlue,
    required this.sealRed,
    required this.paper,
    required this.rice,
    required this.stone,
    required this.ash,
    required this.char,
    required this.riskHigh,
    required this.riskMid,
    required this.riskLow,
    required this.abstain,
    required this.frozen,
  });

  final Brightness brightness;
  final Color inkBlue;
  final Color sealRed;
  final Color paper;
  final Color rice;
  final Color stone;
  final Color ash;
  final Color char;
  final Color riskHigh;
  final Color riskMid;
  final Color riskLow;
  final Color abstain;
  final Color frozen;

  static const StuchkaPalette light = StuchkaPalette(
    brightness: Brightness.light,
    inkBlue: StuchkaTokens.inkBlue,
    sealRed: StuchkaTokens.sealRed,
    paper: StuchkaTokens.paper,
    rice: StuchkaTokens.rice,
    stone: StuchkaTokens.stone,
    ash: StuchkaTokens.ash,
    char: StuchkaTokens.char,
    riskHigh: StuchkaTokens.riskHigh,
    riskMid: StuchkaTokens.riskMid,
    riskLow: StuchkaTokens.riskLow,
    abstain: StuchkaTokens.abstain,
    frozen: StuchkaTokens.frozen,
  );

  static const StuchkaPalette dark = StuchkaPalette(
    brightness: Brightness.dark,
    inkBlue: StuchkaTokens.inkBlueDark,
    sealRed: StuchkaTokens.sealRedDark,
    paper: StuchkaTokens.paperDark,
    rice: StuchkaTokens.riceDark,
    stone: StuchkaTokens.stoneDark,
    ash: StuchkaTokens.ashDark,
    char: StuchkaTokens.charDark,
    riskHigh: StuchkaTokens.riskHigh,
    riskMid: StuchkaTokens.riskMid,
    riskLow: StuchkaTokens.riskLow,
    abstain: StuchkaTokens.abstain,
    frozen: StuchkaTokens.inkBlueDark,
  );
}

/// Semantic colors exposed via `Theme.of(context).extension<StuchkaSemanticColors>()!`
/// (spec 01 §1.6). The only sanctioned route to brand / risk / source colors in business code.
@immutable
class StuchkaSemanticColors extends ThemeExtension<StuchkaSemanticColors> {
  const StuchkaSemanticColors({
    required this.sealRed,
    required this.inkBlue,
    required this.paper,
    required this.rice,
    required this.stone,
    required this.ash,
    required this.char,
    required this.riskHigh,
    required this.riskMid,
    required this.riskLow,
    required this.abstain,
    required this.frozen,
    required this.sourceRule,
    required this.sourceKb,
    required this.sourceOnline,
    required this.sourceInferred,
  });

  final Color sealRed;
  final Color inkBlue;
  final Color paper;
  final Color rice;
  final Color stone;
  final Color ash;
  final Color char;
  final Color riskHigh;
  final Color riskMid;
  final Color riskLow;
  final Color abstain;
  final Color frozen;
  final Color sourceRule;
  final Color sourceKb;
  final Color sourceOnline;
  final Color sourceInferred;

  factory StuchkaSemanticColors.from(StuchkaPalette p) => StuchkaSemanticColors(
        sealRed: p.sealRed,
        inkBlue: p.inkBlue,
        paper: p.paper,
        rice: p.rice,
        stone: p.stone,
        ash: p.ash,
        char: p.char,
        riskHigh: p.riskHigh,
        riskMid: p.riskMid,
        riskLow: p.riskLow,
        abstain: p.abstain,
        frozen: p.frozen,
        sourceRule: StuchkaTokens.sourceRule,
        sourceKb: StuchkaTokens.sourceKb,
        sourceOnline: StuchkaTokens.sourceOnline,
        sourceInferred: StuchkaTokens.sourceInferred,
      );

  static StuchkaSemanticColors of(BuildContext context) =>
      Theme.of(context).extension<StuchkaSemanticColors>()!;

  @override
  StuchkaSemanticColors copyWith({
    Color? sealRed,
    Color? inkBlue,
    Color? paper,
    Color? rice,
    Color? stone,
    Color? ash,
    Color? char,
    Color? riskHigh,
    Color? riskMid,
    Color? riskLow,
    Color? abstain,
    Color? frozen,
    Color? sourceRule,
    Color? sourceKb,
    Color? sourceOnline,
    Color? sourceInferred,
  }) {
    return StuchkaSemanticColors(
      sealRed: sealRed ?? this.sealRed,
      inkBlue: inkBlue ?? this.inkBlue,
      paper: paper ?? this.paper,
      rice: rice ?? this.rice,
      stone: stone ?? this.stone,
      ash: ash ?? this.ash,
      char: char ?? this.char,
      riskHigh: riskHigh ?? this.riskHigh,
      riskMid: riskMid ?? this.riskMid,
      riskLow: riskLow ?? this.riskLow,
      abstain: abstain ?? this.abstain,
      frozen: frozen ?? this.frozen,
      sourceRule: sourceRule ?? this.sourceRule,
      sourceKb: sourceKb ?? this.sourceKb,
      sourceOnline: sourceOnline ?? this.sourceOnline,
      sourceInferred: sourceInferred ?? this.sourceInferred,
    );
  }

  @override
  StuchkaSemanticColors lerp(ThemeExtension<StuchkaSemanticColors>? other, double t) {
    if (other is! StuchkaSemanticColors) return this;
    return StuchkaSemanticColors(
      sealRed: Color.lerp(sealRed, other.sealRed, t)!,
      inkBlue: Color.lerp(inkBlue, other.inkBlue, t)!,
      paper: Color.lerp(paper, other.paper, t)!,
      rice: Color.lerp(rice, other.rice, t)!,
      stone: Color.lerp(stone, other.stone, t)!,
      ash: Color.lerp(ash, other.ash, t)!,
      char: Color.lerp(char, other.char, t)!,
      riskHigh: Color.lerp(riskHigh, other.riskHigh, t)!,
      riskMid: Color.lerp(riskMid, other.riskMid, t)!,
      riskLow: Color.lerp(riskLow, other.riskLow, t)!,
      abstain: Color.lerp(abstain, other.abstain, t)!,
      frozen: Color.lerp(frozen, other.frozen, t)!,
      sourceRule: Color.lerp(sourceRule, other.sourceRule, t)!,
      sourceKb: Color.lerp(sourceKb, other.sourceKb, t)!,
      sourceOnline: Color.lerp(sourceOnline, other.sourceOnline, t)!,
      sourceInferred: Color.lerp(sourceInferred, other.sourceInferred, t)!,
    );
  }
}

/// Spacing scale exposed as a theme extension (spec 01 §1.9).
@immutable
class StuchkaSpacing extends ThemeExtension<StuchkaSpacing> {
  const StuchkaSpacing({
    this.xs = StuchkaSpacingTokens.xs,
    this.sm = StuchkaSpacingTokens.sm,
    this.md = StuchkaSpacingTokens.md,
    this.lg = StuchkaSpacingTokens.lg,
    this.xl = StuchkaSpacingTokens.xl,
    this.xxl = StuchkaSpacingTokens.xxl,
    this.radiusSeal = StuchkaSpacingTokens.radiusSeal,
    this.radiusCard = StuchkaSpacingTokens.radiusCard,
    this.radiusDialog = StuchkaSpacingTokens.radiusDialog,
  });

  final double xs, sm, md, lg, xl, xxl;
  final double radiusSeal, radiusCard, radiusDialog;

  static const StuchkaSpacing standard = StuchkaSpacing();

  static StuchkaSpacing of(BuildContext context) =>
      Theme.of(context).extension<StuchkaSpacing>() ?? standard;

  @override
  StuchkaSpacing copyWith() => this;

  @override
  StuchkaSpacing lerp(ThemeExtension<StuchkaSpacing>? other, double t) => this;
}

/// ThemeData assembly (spec 01 §1.6). Light / dark seeds drawn from tokens.
class StuchkaTheme {
  StuchkaTheme._();

  static ThemeData light() => _build(StuchkaPalette.light);
  static ThemeData dark() => _build(StuchkaPalette.dark);

  static ThemeData _build(StuchkaPalette p) {
    final scheme = ColorScheme(
      brightness: p.brightness,
      primary: p.inkBlue,
      onPrimary: p.paper,
      secondary: p.sealRed,
      onSecondary: p.paper,
      surface: p.paper,
      onSurface: p.char,
      error: p.riskHigh,
      onError: p.paper,
    );
    return ThemeData(
      useMaterial3: true,
      // 思源字体本地打包后填 'SourceHanSans'；R1a 字体资产缺位时回退系统字体。
      fontFamily: null,
      colorScheme: scheme,
      scaffoldBackgroundColor: p.paper,
      textTheme: _textTheme(p),
      extensions: <ThemeExtension<dynamic>>[
        StuchkaSemanticColors.from(p),
        StuchkaSpacing.standard,
      ],
    );
  }

  static TextTheme _textTheme(StuchkaPalette p) {
    final c = p.char;
    return TextTheme(
      displayLarge: TextStyle(fontSize: StuchkaTypography.display, color: c, fontWeight: FontWeight.w700),
      titleLarge: TextStyle(fontSize: StuchkaTypography.title, color: c, fontWeight: FontWeight.w500),
      titleMedium: TextStyle(fontSize: StuchkaTypography.heading, color: c, fontWeight: FontWeight.w500),
      bodyLarge: TextStyle(fontSize: StuchkaTypography.body, color: c),
      bodyMedium: TextStyle(fontSize: StuchkaTypography.body, color: c),
      bodySmall: TextStyle(fontSize: StuchkaTypography.caption, color: p.ash),
      labelLarge: TextStyle(fontSize: StuchkaTypography.body, color: c, fontWeight: FontWeight.w500),
    );
  }
}
