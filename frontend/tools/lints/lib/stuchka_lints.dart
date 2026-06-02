// Stučka custom_lint plugin entry (spec 01 §1.8 防线1).
//
// Exposes two lint rules:
//   - avoid-hardcoded-colors          (spec 01 §1.6): no `Color(0x..)` / `Colors.*` literals in
//                                       business code — use StuchkaSemanticColors / StuchkaTokens.
//   - forbid-emoji-in-string-literal  (spec 01 §1.8): no Emoji codepoints in string literals.
//
// Registered via the analysis_options.yaml `custom_lint` plugin block. If the beta toolchain
// cannot run the plugin deterministically, the test-based defenses (test/lint/*) remain the
// authoritative gate; this plugin is the editor-time + `dart run custom_lint` layer.
library;

import 'package:custom_lint_builder/custom_lint_builder.dart';

import 'src/avoid_hardcoded_colors.dart';
import 'src/forbid_emoji_in_string_literal.dart';

PluginBase createPlugin() => _StuchkaLintsPlugin();

class _StuchkaLintsPlugin extends PluginBase {
  @override
  List<LintRule> getLintRules(CustomLintConfigs configs) => [
        const AvoidHardcodedColors(),
        const ForbidEmojiInStringLiteral(),
      ];
}
