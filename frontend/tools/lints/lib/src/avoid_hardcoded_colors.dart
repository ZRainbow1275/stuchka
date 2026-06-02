// avoid-hardcoded-colors (spec 01 §1.6 / §1.7). Business code must source colors from
// StuchkaSemanticColors / StuchkaTokens, never a raw `Color(0x..)` literal or `Colors.*`.
//
// The generated token file (lib/theme/generated/tokens.g.dart) and the token wrappers themselves
// are the *only* sanctioned writers of Color literals; this rule excludes those paths.
import 'package:analyzer/error/listener.dart';
import 'package:custom_lint_builder/custom_lint_builder.dart';

class AvoidHardcodedColors extends DartLintRule {
  const AvoidHardcodedColors() : super(code: _code);

  static const _code = LintCode(
    name: 'avoid_hardcoded_colors',
    problemMessage:
        'Hardcoded color literal. Use StuchkaSemanticColors / StuchkaTokens (spec 01 §1.6).',
    errorSeverity: ErrorSeverity.WARNING,
  );

  // Files that ARE allowed to write Color literals (token generators / theme plumbing).
  static const _allowedSuffixes = <String>[
    'lib/theme/generated/tokens.g.dart',
    'lib/theme/stuchka_theme.dart',
  ];

  bool _isAllowed(String path) {
    final norm = path.replaceAll(r'\', '/');
    return _allowedSuffixes.any(norm.endsWith);
  }

  @override
  void run(
    CustomLintResolver resolver,
    ErrorReporter reporter,
    CustomLintContext context,
  ) {
    if (_isAllowed(resolver.path)) return;

    // `Color(0x..)` / `Color.fromARGB(...)` / `Color.fromRGBO(...)` constructor calls.
    context.registry.addInstanceCreationExpression((node) {
      final typeName = node.constructorName.type.name2.lexeme;
      if (typeName == 'Color') {
        reporter.atNode(node, _code);
      }
    });

    // `Colors.*` (Material palette) property access.
    context.registry.addPrefixedIdentifier((node) {
      if (node.prefix.name == 'Colors') {
        reporter.atNode(node, _code);
      }
    });
  }
}
