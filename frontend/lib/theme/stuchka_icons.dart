import 'package:flutter/widgets.dart';
import 'package:flutter_lucide/flutter_lucide.dart';

/// The唯一图标入口 — Lucide line icons only (spec 01 §1.7).
///
/// Any Material `Icons.*` / Cupertino `CupertinoIcons.*` is forbidden (review-reject + the
/// `tools/lint_no_emoji.dart` plus an `Icons.`/`CupertinoIcons.` grep gate). Every glyph here is
/// a `LucideIcons` constant; if an upstream name changes the compile fails here loudly (one place).
class StuchkaIcons {
  StuchkaIcons._();

  // workbench / diagnosis
  static const IconData diagnose = LucideIcons.stethoscope;
  static const IconData evidence = LucideIcons.file_search;
  static const IconData calc = LucideIcons.calculator;
  static const IconData deadline = LucideIcons.alarm_clock;
  static const IconData freeze = LucideIcons.snowflake;
  static const IconData arbitrate = LucideIcons.gavel;
  static const IconData document = LucideIcons.file_text;
  static const IconData procedure = LucideIcons.git_branch;
  static const IconData audit = LucideIcons.scroll_text;
  static const IconData identity = LucideIcons.id_card;

  // accessibility / crisis
  static const IconData heart = LucideIcons.heart;
  static const IconData alert = LucideIcons.triangle_alert;
  static const IconData help = LucideIcons.life_buoy;

  // tree node kinds
  static const IconData caseRoot = LucideIcons.folder;
  static const IconData factGroup = LucideIcons.list_checks;
  static const IconData evidenceGroup = LucideIcons.paperclip;
  static const IconData documentGroup = LucideIcons.files;
  static const IconData calculationGroup = LucideIcons.sigma;
  static const IconData deadlineGroup = LucideIcons.timer;

  // source tags
  static const IconData sourceRule = LucideIcons.sigma;
  static const IconData sourceKb = LucideIcons.database;
  static const IconData sourceOnline = LucideIcons.globe;
  static const IconData sourceInferred = LucideIcons.sparkles;

  // generic
  static const IconData check = LucideIcons.check;
  static const IconData close = LucideIcons.x;
  static const IconData add = LucideIcons.plus;
  static const IconData refresh = LucideIcons.refresh_cw;
  static const IconData chevronRight = LucideIcons.chevron_right;
  static const IconData chevronDown = LucideIcons.chevron_down;
  static const IconData info = LucideIcons.info;
  static const IconData sun = LucideIcons.sun;
  static const IconData moon = LucideIcons.moon;
  static const IconData zoomIn = LucideIcons.zoom_in;
  static const IconData phone = LucideIcons.phone;
  static const IconData signature = LucideIcons.pen_line;
  static const IconData download = LucideIcons.download;
  static const IconData cloud = LucideIcons.cloud;
  static const IconData cloudOff = LucideIcons.cloud_off;
  static const IconData lock = LucideIcons.lock;
  static const IconData send = LucideIcons.send;
}
