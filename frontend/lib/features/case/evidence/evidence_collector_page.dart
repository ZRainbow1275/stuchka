import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../ipc/dto/dtos.dart';
import '../../../ipc/dto/enums.dart';
import '../../../ipc/rust_core_client.dart';
import '../../../ipc/rust_core_providers.dart';
import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';
import '../case_providers.dart';
import 'effective_score_widget.dart';
import 'evidence_cards.dart';
import 'recording_consent_gate.dart';
import 'scene_card.dart';

/// /case/:id/evidence — the seven-class evidence collector. A card per category + a REAL desktop
/// file import (`file_selector`) whose actual bytes are uploaded (`POST /case/:id/evidence`
/// multipart), the EffectiveScore four-band readout, and a SceneCard showing the file's real
/// EXIF/GPS + the backend SHA-256. Audio/video imports pass the 法律 P5 legality-consent gate first.
/// In-app phone CAPTURE (camera/mic) is an explicit R2 seam — desktop import only here, never faked.
class EvidenceCollectorPage extends ConsumerStatefulWidget {
  const EvidenceCollectorPage({super.key, required this.caseId});
  final String caseId;

  @override
  ConsumerState<EvidenceCollectorPage> createState() => _EvidenceCollectorPageState();
}

class _EvidenceCollectorPageState extends ConsumerState<EvidenceCollectorPage> {
  EvidenceCategory _selected = EvidenceCategory.documentaryContract;
  Map<String, dynamic> _metadata = {};
  bool _busy = false;
  String? _error;
  EvidenceDto? _result;
  SceneMeta? _scene;

  EvidenceCardSpec get _spec =>
      EvidenceCardSpec.all.firstWhere((s) => s.category == _selected);

  /// File-type groups per category (the picker still allows "所有文件" as the last group so an
  /// uncommon container is never blocked).
  List<XTypeGroup> _typeGroups() {
    const any = XTypeGroup(label: '所有文件');
    switch (_selected) {
      case EvidenceCategory.audioVideo:
        return const [
          XTypeGroup(label: '录音 / 录像', extensions: ['mp3', 'm4a', 'wav', 'amr', 'aac', 'mp4', 'mov', 'mkv', 'avi']),
          any,
        ];
      case EvidenceCategory.scenePhotoVideo:
        return const [
          XTypeGroup(label: '照片 / 视频', extensions: ['jpg', 'jpeg', 'png', 'heic', 'webp', 'mp4', 'mov']),
          any,
        ];
      case EvidenceCategory.documentaryContract:
      case EvidenceCategory.appraisal:
        return const [
          XTypeGroup(label: '文档', extensions: ['pdf', 'doc', 'docx', 'txt', 'jpg', 'jpeg', 'png']),
          any,
        ];
      default:
        return const [any];
    }
  }

  Future<void> _import() async {
    setState(() => _error = null);

    // 1. REAL file pick (desktop). Cancel -> abort silently.
    final XFile? file = await openFile(acceptedTypeGroups: _typeGroups());
    if (file == null) return;

    // 2. 法律 P5 legality-consent gate for audio/video evidence (blocks until acknowledged).
    if (_selected == EvidenceCategory.audioVideo) {
      if (!mounted) return;
      final consented = await RecordingConsentGate.show(context);
      if (!consented) {
        setState(() => _error = '已取消：未确认录音 / 录像的合法取得方式，故未导入。');
        return;
      }
    }

    setState(() => _busy = true);
    try {
      // 3. Read the REAL bytes (no placeholder/fabricated content).
      final bytes = await file.readAsBytes();

      // 4. Extract real scene metadata (EXIF/GPS) for photo/video; identity-only for others.
      final scene = await extractScene(file.name, bytes);

      // 5. Merge the genuine provenance into the upload metadata for backend scoring/audit.
      final meta = <String, dynamic>{
        ..._metadata,
        'sourceFileName': file.name,
        'importChannel': 'desktop_file_import',
        if (scene.capturedAt != null) 'exifCapturedAt': scene.capturedAt,
        if (scene.hasGps) 'exifGpsLat': scene.latitude,
        if (scene.hasGps) 'exifGpsLon': scene.longitude,
        if (_selected == EvidenceCategory.audioVideo) 'recordingConsent': true,
      };

      final ev = await client.uploadEvidence(
        widget.caseId,
        fileBytes: bytes,
        fileName: file.name,
        metadata: meta,
      );
      setState(() {
        _result = ev;
        _scene = scene;
      });
      ref.invalidate(caseAggregateProvider(widget.caseId));
    } on RustCoreApiException catch (e) {
      setState(() => _error = '${e.code}: ${e.message}');
    } catch (e) {
      setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  RustCoreClient get client => ref.read(rustCoreClientProvider);

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('证据收集 · 七大类')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: EvidenceCategory.values.map((c) {
              return ChoiceChip(
                selected: _selected == c,
                label: Text(c.labelZh),
                onSelected: (_) => setState(() {
                  _selected = c;
                  _metadata = {};
                  _result = null;
                  _scene = null;
                  _error = null;
                }),
              );
            }).toList(),
          ),
          const SizedBox(height: 12),
          EvidenceCardForm(
            key: ValueKey(_selected),
            spec: _spec,
            onChanged: (m) => _metadata = m,
          ),
          const SizedBox(height: 12),
          FilledButton.icon(
            onPressed: _busy ? null : _import,
            icon: _busy
                ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))
                : const Icon(StuchkaIcons.evidence),
            label: const Text('选择文件并上传评分'),
          ),
          const SizedBox(height: 8),
          // Honest R2 seam: phone capture is not built on desktop — say so, never fake a camera.
          Text(
            '手机端拍摄 / 录音取证为后续版本（R2）能力；当前桌面端支持从本机导入已有文件。',
            style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: StuchkaSemanticColors.of(context).inkBlue,
                ),
          ),
          if (_error != null) ...[
            const SizedBox(height: 12),
            Text(
              _error!,
              style: TextStyle(color: StuchkaSemanticColors.of(context).riskHigh),
            ),
          ],
          if (_scene != null) ...[
            const SizedBox(height: 16),
            SceneCard(scene: _scene!, sha256: _result?.fileSha256),
          ],
          if (_result != null) ...[
            const SizedBox(height: 12),
            Card(
              child: Padding(
                padding: const EdgeInsets.all(12),
                child: EffectiveScoreWidget(
                  score: _result!.effectiveScore,
                  breakdown: _result!.scoreBreakdown,
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }
}
