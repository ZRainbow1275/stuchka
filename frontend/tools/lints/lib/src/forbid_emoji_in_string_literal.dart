// forbid-emoji-in-string-literal (spec 01 §1.8 防线1). No Emoji codepoints anywhere in a string
// literal. The Unicode blocks mirror tools/lint_no_emoji.dart so the editor-time rule and the CI
// scanner agree:
//   U+1F300–U+1FAFF, U+2600–U+27BF, U+1F000–U+1F2FF, plus the regional-indicator / dingbat ranges.
import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/error/listener.dart';
import 'package:custom_lint_builder/custom_lint_builder.dart';

class ForbidEmojiInStringLiteral extends DartLintRule {
  const ForbidEmojiInStringLiteral() : super(code: _code);

  static const _code = LintCode(
    name: 'forbid_emoji_in_string_literal',
    problemMessage: 'Emoji are forbidden (spec 01 §1.8). Use Lucide icons + text instead.',
    errorSeverity: ErrorSeverity.ERROR,
  );

  static bool _isEmoji(int cp) {
    return (cp >= 0x1F300 && cp <= 0x1FAFF) ||
        (cp >= 0x2600 && cp <= 0x27BF) ||
        (cp >= 0x1F000 && cp <= 0x1F2FF) ||
        (cp >= 0x1FA70 && cp <= 0x1FAFF) ||
        cp == 0x2B50 ||
        cp == 0x2B55 ||
        (cp >= 0x1F1E6 && cp <= 0x1F1FF) || // regional indicators
        cp == 0xFE0F; // variation selector-16 (emoji presentation)
  }

  bool _hasEmoji(String s) => s.runes.any(_isEmoji);

  @override
  void run(
    CustomLintResolver resolver,
    ErrorReporter reporter,
    CustomLintContext context,
  ) {
    context.registry.addSimpleStringLiteral((node) {
      if (_hasEmoji(node.value)) reporter.atNode(node, _code);
    });

    context.registry.addStringInterpolation((node) {
      for (final element in node.elements) {
        if (element is InterpolationString && _hasEmoji(element.value)) {
          reporter.atNode(node, _code);
          return;
        }
      }
    });
  }
}
