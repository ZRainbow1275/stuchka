import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:webview_windows/webview_windows.dart';

import '../../../bridge/bridge_dispatcher.dart';
import '../../../bridge/bridge_ime_observer.dart';
import '../../../ipc/rust_core_client.dart';
import '../../../ipc/rust_core_providers.dart';
import '../../../shell/session_actor.dart';
import '../../../shell/webview_host.dart';
import '../../../shell/webview_tab_router.dart';
import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';
import '../../accessibility/crisis/crisis_level3_dialog.dart';
import '../../accessibility/high_risk/disclaimer_payload.dart';
import '../../accessibility/high_risk/high_risk_ack.dart';
import '../../accessibility/high_risk/high_risk_gate.dart';
import '../../editor/editor_controller.dart';

/// /case/:id/documents — the live WebView2 host for the Tiptap editor (Channel A) + the document-side
/// INV-10 high-risk trigger sites (compliance/05 §1 S-03 放弃请求项 / S-05 刑事报案材料导出).
///
/// On mount it creates a Yjs document (`POST /case/:id/document`), boots a [WebViewEditorHost] that
/// loads `assets/web/editor/index.html` (CSP `connect-src 'none'`), and wires the Channel-A bridge:
/// a [BridgeDispatcher] fans inbound envelopes into the [EditorStepRelayController] (relays
/// `editor.step` -> `POST /document/:id/steps`, INV-06 double-write), the [WebViewTabRouter], and the
/// [BridgeImeObserver]. The query params steer the migrant-wage scenario (spec §4.9).
class DocumentHostPage extends ConsumerStatefulWidget {
  const DocumentHostPage({
    super.key,
    required this.caseId,
    this.templates = const [],
    this.exportRequested = false,
  });

  final String caseId;

  /// Preselected document template ids (from `?templates=tpl_inspection,tpl_arbitration`).
  final List<String> templates;

  /// Whether the page was opened with `?action=export` (migrant-wage 导出卷宗 step).
  final bool exportRequested;

  /// The editor bundle assets loaded into the WebView host (declared in pubspec assets).
  static const String editorBundleAsset = 'assets/web/editor/stuchka-editor.umd.js';
  static const String editorIndexAsset = 'assets/web/editor/index.html';

  @override
  ConsumerState<DocumentHostPage> createState() => _DocumentHostPageState();
}

class _DocumentHostPageState extends ConsumerState<DocumentHostPage> {
  final GlobalKey _webViewKey = GlobalKey();

  WebViewEditorHost? _host;
  BridgeDispatcher? _dispatcher;
  EditorStepRelayController? _relay;
  WebViewTabRouter? _tabRouter;
  BridgeImeObserver? _ime;

  bool _booting = true;
  Object? _bootError;
  String? _docId;

  /// Set while a CJK IME composition is in progress inside the WebView (caret in GLOBAL coords). The
  /// shell uses this to avoid stealing focus / firing shortcuts mid-composition (FE02 §2.7).
  Rect? _composingCaret;

  @override
  void initState() {
    super.initState();
    _boot();
  }

  Future<void> _boot() async {
    // Declared up-front (not as success-path locals) so a throw AFTER host.initialize() — e.g.
    // tabRouter.switchTo -> dispatcher.send -> host.postWebMessage, a live WebView2 platform call —
    // can release the partially-built graph in the catch instead of leaking the WebView2 controller
    // + every StreamController/subscription (the fields are only assigned in the success setState).
    WebViewEditorHost? host;
    BridgeDispatcher? dispatcher;
    EditorStepRelayController? relay;
    WebViewTabRouter? tabRouter;
    BridgeImeObserver? ime;
    try {
      final client = ref.read(rustCoreClientProvider);
      // R1 documents are created server-side (Yjs doc + main-store row); arb_application is the safe
      // default template (the editor can switch templates afterwards).
      final docId = await client.createDocument(widget.caseId);
      String kbHash = '';
      try {
        kbHash = (await client.kbVersion()).versionHash;
      } catch (_) {
        // KB freshness is non-fatal for booting the editor; leave kbHash empty if unavailable.
      }
      host = WebViewEditorHost(caseId: widget.caseId, docId: docId, kbHash: kbHash);
      await host.initialize();
      dispatcher = BridgeDispatcher(host);
      relay = EditorStepRelayController(
        dispatcher: dispatcher,
        client: client,
        actor: sessionActorId,
      );
      tabRouter = WebViewTabRouter(dispatcher);
      ime = BridgeImeObserver(
        dispatcher,
        webViewOrigin: _webViewOrigin,
        setCaret: _onCaret,
      );
      await tabRouter.switchTo(tabId: docId, docId: docId, role: 'editor');
      if (!mounted) {
        await _disposeBootGraph(host, dispatcher, relay, tabRouter, ime);
        return;
      }
      setState(() {
        _host = host;
        _dispatcher = dispatcher;
        _relay = relay;
        _tabRouter = tabRouter;
        _ime = ime;
        _docId = docId;
        _booting = false;
      });
    } catch (e) {
      // Release whatever was built before the failure (mirrors the !mounted cancellation path).
      await _disposeBootGraph(host, dispatcher, relay, tabRouter, ime);
      if (mounted) {
        setState(() {
          _bootError = e;
          _booting = false;
        });
      }
    }
  }

  /// Tear down a partially-built boot graph (used by both the `!mounted` cancellation and the boot
  /// failure paths) so a failed boot never leaks the WebView2 process or open streams.
  Future<void> _disposeBootGraph(
    WebViewEditorHost? host,
    BridgeDispatcher? dispatcher,
    EditorStepRelayController? relay,
    WebViewTabRouter? tabRouter,
    BridgeImeObserver? ime,
  ) async {
    await ime?.dispose();
    await relay?.dispose();
    await tabRouter?.dispose();
    await dispatcher?.dispose();
    await host?.dispose();
  }

  Offset _webViewOrigin() {
    final box = _webViewKey.currentContext?.findRenderObject() as RenderBox?;
    return box?.localToGlobal(Offset.zero) ?? Offset.zero;
  }

  void _onCaret(Rect caret) {
    if (mounted) setState(() => _composingCaret = caret);
  }

  @override
  void dispose() {
    _ime?.dispose();
    _relay?.dispose();
    _tabRouter?.dispose();
    _dispatcher?.dispose();
    _host?.dispose();
    super.dispose();
  }

  /// S-03 放弃任一仲裁请求项 (INV-10 real trigger).
  Future<void> _abandonClaim() async {
    final timings = await HighRiskGate.show(
      context,
      HighRiskScenario.abandonClaim,
      payload: const DisclaimerPayload(
        requestedItemName: '未签劳动合同二倍工资差额',
        amount: '48000',
        confidence: '较高',
      ),
    );
    if (timings == null) return;
    await postHighRiskAck(
      ref,
      caseId: widget.caseId,
      scenario: HighRiskScenario.abandonClaim,
      timings: timings,
    );
  }

  /// S-05 刑事报案材料导出（拒不支付劳动报酬罪）(INV-10 real trigger). After the audited two-step
  /// acknowledgement, this PERFORMS the real GB 45438 dossier export (`POST /document/:id/export`):
  /// success is reported only on a genuine backend result — never a fabricated "exported" message.
  Future<void> _exportCriminalReport() async {
    final timings = await HighRiskGate.show(context, HighRiskScenario.criminalReportExport);
    if (timings == null) return;
    await postHighRiskAck(
      ref,
      caseId: widget.caseId,
      scenario: HighRiskScenario.criminalReportExport,
      timings: timings,
    );
    if (!mounted) return;
    final messenger = ScaffoldMessenger.of(context);
    final docId = _docId;
    if (docId == null) {
      messenger.showSnackBar(
        const SnackBar(content: Text('文书内核尚未就绪，无法导出刑事报案材料。')),
      );
      return;
    }
    try {
      final resp = await ref.read(rustCoreClientProvider).exportDocument(docId);
      if (!mounted) return;
      messenger.showSnackBar(
        SnackBar(
          content: Text('刑事报案材料已导出：${resp.entries.length} 个文件 '
              '(${resp.dossierZipPath})；前置程序由您自行核实。'),
        ),
      );
    } on RustCoreApiException catch (e) {
      if (!mounted) return;
      messenger.showSnackBar(SnackBar(content: Text('导出失败：${e.message}')));
    } catch (e) {
      if (!mounted) return;
      messenger.showSnackBar(SnackBar(content: Text('导出失败：$e')));
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('文书工作台'),
        bottom: _composingCaret != null
            ? const PreferredSize(
                preferredSize: Size.fromHeight(2),
                child: LinearProgressIndicator(minHeight: 2),
              )
            : null,
      ),
      body: Column(
        children: [
          // INV-07 Level-3 24h cooldown banner (compliance/05 §5.2).
          const Level3CooldownGate(),
          Expanded(child: _editorArea(context)),
          _inv10Bar(context),
        ],
      ),
    );
  }

  Widget _editorArea(BuildContext context) {
    if (_bootError != null) {
      return _centeredCard(
        context,
        icon: StuchkaIcons.alert,
        title: '编辑器加载失败',
        body: '无法启动 WebView2 文书编辑器：$_bootError\n\n'
            'WebView2 运行时为 Windows R1 必需组件；请确认已安装 Microsoft Edge WebView2 Runtime。',
      );
    }
    if (_booting || _host == null) {
      return _centeredCard(
        context,
        icon: StuchkaIcons.document,
        title: '文书编辑器加载中',
        body: '正在创建 Yjs 文书并加载 WebView2 编辑器内核'
            '（assets/web/editor/index.html，CSP connect-src \'none\'）。',
      );
    }
    // Live WebView2 editor. The key lets the IME observer resolve the WebView's global origin.
    return Padding(
      padding: const EdgeInsets.all(8),
      child: ClipRRect(
        key: _webViewKey,
        borderRadius: BorderRadius.circular(6),
        child: Webview(_host!.controller),
      ),
    );
  }

  Widget _centeredCard(
    BuildContext context, {
    required IconData icon,
    required String title,
    required String body,
  }) {
    final colors = StuchkaSemanticColors.of(context);
    final card = ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 560),
      child: Card(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(icon, size: 40, color: colors.inkBlue),
              const SizedBox(height: 12),
              Text(title, style: Theme.of(context).textTheme.titleLarge),
              const SizedBox(height: 8),
              Text(body, textAlign: TextAlign.center),
              const SizedBox(height: 16),
              Container(
                padding: const EdgeInsets.all(8),
                decoration: BoxDecoration(
                  color: colors.rice,
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Text(
                  _docId == null ? '案件：${widget.caseId}' : '案件：${widget.caseId} · 文书：$_docId',
                  style: const TextStyle(fontFamily: 'JetBrainsMonoSC'),
                ),
              ),
              if (widget.templates.isNotEmpty) ...[
                const SizedBox(height: 8),
                Text('预选模板：${widget.templates.join(' / ')}',
                    style: Theme.of(context).textTheme.bodySmall),
              ],
            ],
          ),
        ),
      ),
    );
    // Scrollable so the card stays vertically centred when there is room yet never overflows a
    // short surface (e.g. a router smoke test that pumps this page without a backend client).
    return LayoutBuilder(
      builder: (context, constraints) => SingleChildScrollView(
        child: ConstrainedBox(
          constraints: BoxConstraints(minHeight: constraints.maxHeight),
          child: Center(
            child: Padding(padding: const EdgeInsets.all(24), child: card),
          ),
        ),
      ),
    );
  }

  Widget _inv10Bar(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    return Material(
      elevation: 2,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
        child: Wrap(
          spacing: 12,
          runSpacing: 8,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            Text('高风险文书操作', style: Theme.of(context).textTheme.titleSmall),
            OutlinedButton.icon(
              icon: Icon(StuchkaIcons.close, size: 16, color: colors.riskHigh),
              label: const Text('放弃某项仲裁请求'),
              onPressed: _abandonClaim,
            ),
            FilledButton.icon(
              icon: const Icon(StuchkaIcons.download, size: 16),
              label: Text(widget.exportRequested ? '导出刑事报案材料（待确认）' : '导出刑事报案材料'),
              style: FilledButton.styleFrom(backgroundColor: colors.sealRed),
              onPressed: _exportCriminalReport,
            ),
          ],
        ),
      ),
    );
  }
}
