import 'package:flutter/material.dart';

import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';

/// /case/:id/documents — the WebView2 host for the Tiptap editor (channel A).
///
/// R1a boundary (brief §6.1 / §7): the `webview_windows` package is P0-blocked on Dart 3 and the
/// editor内核 belongs to the 03-editor-workbench subtask. The shell ships the host SCAFFOLD + a
/// document-route placeholder that names the bundle it will load
/// (assets/web/editor/stuchka-editor.umd.js) and the channel-A CSP invariant. The live WebView load
/// test is gated until a Dart-3-compatible webview package is wired.
class DocumentHostPage extends StatelessWidget {
  const DocumentHostPage({super.key, required this.caseId});
  final String caseId;

  /// The editor bundle asset path (loaded into the WebView host once a webview package is wired).
  static const String editorBundleAsset = 'assets/web/editor/stuchka-editor.umd.js';
  static const String editorIndexAsset = 'assets/web/editor/index.html';

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    return Scaffold(
      appBar: AppBar(title: const Text('文书工作台')),
      body: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 520),
          child: Card(
            child: Padding(
              padding: const EdgeInsets.all(24),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(StuchkaIcons.document, size: 40, color: colors.inkBlue),
                  const SizedBox(height: 12),
                  Text('文书编辑器加载中', style: Theme.of(context).textTheme.titleLarge),
                  const SizedBox(height: 8),
                  const Text(
                    '编辑器内核（Tiptap / ProseMirror）将在 WebView2 宿主中加载 '
                    'assets/web/editor/stuchka-editor.umd.js。\n\n'
                    '通道 A（WebView↔编辑器）的 CSP 为 connect-src \'none\'，永不直接触达 Rust core；'
                    'ProseMirror Step 由外壳经通道 B 转交持久化。\n\n'
                    '编辑器内核装配属 03-editor-workbench 子任务。',
                    textAlign: TextAlign.center,
                  ),
                  const SizedBox(height: 16),
                  Container(
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: colors.rice,
                      borderRadius: BorderRadius.circular(4),
                    ),
                    child: Text('案件：$caseId',
                        style: const TextStyle(fontFamily: 'JetBrainsMonoSC')),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
