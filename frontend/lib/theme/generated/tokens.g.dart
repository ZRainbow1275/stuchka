// GENERATED FILE — do not edit by hand.
// Regenerate via: dart run tools/gen_tokens.dart
// ignore_for_file: public_member_api_docs
import 'dart:ui';

class StuchkaTokens {
  StuchkaTokens._();
  static const Color sealRed = Color(0xFFA33A2F);
  static const Color sealRedDeep = Color(0xFF7E2A23);
  static const Color sealRedSoft = Color(0xFFD8615A);
  static const Color inkBlue = Color(0xFF1F3A5F);
  static const Color inkBlueDeep = Color(0xFF13243D);
  static const Color inkBlueSoft = Color(0xFF3F5A85);
  static const Color paper = Color(0xFFFBF8F2);
  static const Color rice = Color(0xFFF2EDE2);
  static const Color stone = Color(0xFFD9D4C7);
  static const Color ash = Color(0xFF7F7A6F);
  static const Color char = Color(0xFF2A2823);
  static const Color riskHigh = Color(0xFFB0392B);
  static const Color riskMid = Color(0xFFC77A3A);
  static const Color riskLow = Color(0xFF3F7A4F);
  static const Color abstain = Color(0xFF5B5B5B);
  static const Color frozen = Color(0xFF1F3A5F);
  static const Color sourceRule = Color(0xFF1F3A5F);
  static const Color sourceKb = Color(0xFF3F7A4F);
  static const Color sourceOnline = Color(0xFFC77A3A);
  static const Color sourceInferred = Color(0xFF7E2A23);
  static const Color paperDark = Color(0xFF1A1F26);
  static const Color riceDark = Color(0xFF222831);
  static const Color stoneDark = Color(0xFF323A45);
  static const Color ashDark = Color(0xFF9AA0A8);
  static const Color charDark = Color(0xFFE6E3DC);
  static const Color sealRedDark = Color(0xFFC45248);
  static const Color inkBlueDark = Color(0xFF7595C2);
}

class StuchkaTypography {
  StuchkaTypography._();
  static const double display = 28;
  static const double title = 22;
  static const double heading = 18;
  static const double body = 15;
  static const double caption = 13;
  static const double mono = 13;
}

class StuchkaSpacingTokens {
  StuchkaSpacingTokens._();
  static const double xs = 4;
  static const double sm = 8;
  static const double md = 12;
  static const double lg = 16;
  static const double xl = 24;
  static const double xxl = 32;
  static const double radiusSeal = 4;
  static const double radiusCard = 8;
  static const double radiusDialog = 12;
}

/// GB 45438-2025 公文打印 token (spec 01 §1.1 / §1.10). Print is platform-independent and
/// has no dark override; the final PDF + four-layer watermark are owned by the Rust
/// crates/document (宪法 D5) — these constants drive the editor-side preview only.
class StuchkaPrintTokens {
  StuchkaPrintTokens._();
  // text colors
  static const Color textBody = Color(0xFF000000);
  static const Color textHeading = Color(0xFF000000);
  static const Color textMuted = Color(0xFF3A3A3A);
  static const Color textSeal = Color(0xFFA33A2F);
  static const Color ruleLine = Color(0xFF000000);
  // page colors
  static const Color pageBackground = Color(0xFFFFFFFF);
  static const Color headerBand = Color(0xFFFFFFFF);
  static const Color watermarkHint = Color(0xFFC8C8C8);
  // typography
  static const String fontFamilyBody = 'SourceHanSerif';
  static const String fontFamilyHeading = 'SourceHanSerif';
  static const String fontFamilyMono = 'JetBrainsMonoSC';
  static const String fontFamilySeal = 'SourceHanSerif';
  static const double sizeTitle = 22;
  static const double sizeHeading1 = 18;
  static const double sizeHeading2 = 16;
  static const double sizeBody = 15;
  static const double sizeFootnote = 12;
  static const double sizeHeaderLabel = 10.5;
  static const double lineHeight = 1.5;
  static const double firstLineIndentEm = 2;
  // page geometry (mm)
  static const double pageWidthMm = 210;
  static const double pageHeightMm = 297;
  static const double marginTopMm = 25;
  static const double marginBottomMm = 25;
  static const double marginLeftMm = 20;
  static const double marginRightMm = 20;
  static const double headerHeightMm = 12;
  static const double footerHeightMm = 10;
  // GB 45438 四层标识打印参数 (compliance/01 唯一权威)
  static const String gbHeaderLabelText = '本文书由 Stučka 辅助生成，不构成法律意见';
  static const String gbHeaderLabelAlign = 'center';
  static const bool gbPerPageHeader = true;
  static const bool gbFirstPageDeclaration = true;
  static const bool gbYieldsToIdentityHeader = true;
}
