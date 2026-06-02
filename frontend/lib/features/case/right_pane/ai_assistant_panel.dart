import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../ipc/dto/dtos.dart';
import '../../../ipc/dto/enums.dart';
import '../../../ipc/rust_core_client.dart';
import '../../../ipc/rust_core_providers.dart';
import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';
import '../../accessibility/crisis/crisis_banners.dart';
import '../../accessibility/crisis/crisis_detector.dart';
import '../../accessibility/crisis/crisis_level3_dialog.dart';
import 'degrade.dart';

/// One AI message (with its mandatory source chip).
class AiMessage {
  AiMessage({required this.content, required this.sourceTag, this.confidence, this.followups = const []});
  final String content;
  final SourceTag sourceTag;
  final double? confidence;
  final List<String> followups;
}

/// Right pane AI panel (spec 04 §4.5 / brief §3.4). Degrade banner + messages (each with a source
/// chip) + INV-07 inline notice + input composer (`POST /llm/query`).
class AiAssistantPanel extends ConsumerStatefulWidget {
  const AiAssistantPanel({super.key, required this.caseId, this.degradeLevel = DegradeLevel.level0});
  final String caseId;
  final DegradeLevel degradeLevel;

  @override
  ConsumerState<AiAssistantPanel> createState() => _AiAssistantPanelState();
}

class _AiAssistantPanelState extends ConsumerState<AiAssistantPanel> {
  final _input = TextEditingController();
  final _detector = const CrisisDetector();
  final List<AiMessage> _messages = [];
  CrisisLevel _crisis = CrisisLevel.none;
  bool _busy = false;

  @override
  void dispose() {
    _input.dispose();
    super.dispose();
  }

  Future<void> _send() async {
    final text = _input.text.trim();
    if (text.isEmpty) return;
    final lvl = _detector.scan(text);
    setState(() => _crisis = lvl);
    // INV-07 三级响应 (compliance/05 §4.2 / §9.2): severe -> Level-3 (24h cooldown + appeal);
    // mid -> Level-2 soft reminder. Severe is NOT treated as mid.
    if (lvl == CrisisLevel.severe) {
      await CrisisLevel3Dialog.show(context, ref);
    } else if (lvl == CrisisLevel.mid) {
      await CrisisLevel2Dialog.show(context);
    }
    setState(() => _busy = true);
    final client = ref.read(rustCoreClientProvider);
    try {
      final resp = await client.llmQuery(LlmQueryReq(caseId: widget.caseId, prompt: text));
      setState(() {
        _messages.add(AiMessage(
          content: resp.content,
          sourceTag: resp.sourceTag,
          confidence: resp.confidence,
          followups: resp.heuristicFollowups,
        ));
        _input.clear();
      });
    } on RustCoreApiException catch (e) {
      setState(() => _messages.add(AiMessage(
            content: '请求失败 ${e.code}: ${e.message}',
            sourceTag: SourceTag.rule,
          )));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        AiDegradeBanner(level: widget.degradeLevel, affectedModuleCount: widget.degradeLevel.index),
        if (_crisis == CrisisLevel.light)
          CrisisLevel1Banner(onDismiss: () => setState(() => _crisis = CrisisLevel.none)),
        Expanded(
          child: _messages.isEmpty
              ? const Center(child: Text('向 AI 提问 · 每条回答都会标注来源'))
              : ListView.builder(
                  padding: const EdgeInsets.all(12),
                  itemCount: _messages.length,
                  itemBuilder: (context, i) => _AiMessageTile(message: _messages[i]),
                ),
        ),
        _Composer(controller: _input, busy: _busy, onSend: _send),
      ],
    );
  }
}

class _AiMessageTile extends StatelessWidget {
  const _AiMessageTile({required this.message});
  final AiMessage message;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            AiSourceChip(tag: message.sourceTag, confidence: message.confidence),
            const SizedBox(height: 8),
            Text(message.content),
            if (message.followups.isNotEmpty) ...[
              const SizedBox(height: 8),
              const Text('启发式追问（勾选后回填事实卡）：',
                  style: TextStyle(fontWeight: FontWeight.w500)),
              ...message.followups.map((f) => _FollowupCheck(text: f)),
            ],
          ],
        ),
      ),
    );
  }
}

/// Mandatory per-message source chip mapping backend [SourceTag] (rule/kb/online/inferred).
class AiSourceChip extends StatelessWidget {
  const AiSourceChip({super.key, required this.tag, this.confidence});
  final SourceTag tag;
  final double? confidence;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final (color, icon) = switch (tag) {
      SourceTag.rule => (colors.sourceRule, StuchkaIcons.sourceRule),
      SourceTag.kb => (colors.sourceKb, StuchkaIcons.sourceKb),
      SourceTag.online => (colors.sourceOnline, StuchkaIcons.sourceOnline),
      SourceTag.inferred => (colors.sourceInferred, StuchkaIcons.sourceInferred),
    };
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(4),
        border: Border.all(color: color, width: 0.6),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 13, color: color),
          const SizedBox(width: 4),
          Text(tag.labelZh, style: TextStyle(color: color, fontSize: 12)),
          if (confidence != null) ...[
            const SizedBox(width: 6),
            Text('${(confidence! * 100).toStringAsFixed(0)}%',
                style: TextStyle(color: color, fontSize: 12)),
          ],
        ],
      ),
    );
  }
}

class _FollowupCheck extends StatefulWidget {
  const _FollowupCheck({required this.text});
  final String text;
  @override
  State<_FollowupCheck> createState() => _FollowupCheckState();
}

class _FollowupCheckState extends State<_FollowupCheck> {
  bool _v = false;
  @override
  Widget build(BuildContext context) {
    return CheckboxListTile(
      value: _v,
      contentPadding: EdgeInsets.zero,
      controlAffinity: ListTileControlAffinity.leading,
      dense: true,
      onChanged: (v) => setState(() => _v = v ?? false),
      title: Text(widget.text),
    );
  }
}

class _Composer extends StatelessWidget {
  const _Composer({required this.controller, required this.busy, required this.onSend});
  final TextEditingController controller;
  final bool busy;
  final VoidCallback onSend;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(8),
      child: Row(
        children: [
          Expanded(
            child: TextField(
              controller: controller,
              minLines: 1,
              maxLines: 3,
              decoration: const InputDecoration(
                hintText: '向 AI 提问…',
                border: OutlineInputBorder(),
                isDense: true,
              ),
            ),
          ),
          const SizedBox(width: 8),
          IconButton.filled(
            onPressed: busy ? null : onSend,
            icon: const Icon(StuchkaIcons.send),
          ),
        ],
      ),
    );
  }
}
