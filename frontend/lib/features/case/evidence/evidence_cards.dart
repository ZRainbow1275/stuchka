import 'package:flutter/material.dart';

import '../../../ipc/dto/enums.dart';
import '../../../theme/stuchka_theme.dart';

/// One required field on an evidence card.
class EvidenceField {
  const EvidenceField({required this.key, required this.label, this.highSensitive = false});
  final String key;
  final String label;
  final bool highSensitive;
}

/// The seven-class evidence card contract (spec 04 §4.6 / brief §3.5). Each card declares its
/// backend [EvidenceCategory] + required fields + a help hint. The seven concrete cards are built
/// from this single descriptor (the unified-interface requirement).
class EvidenceCardSpec {
  const EvidenceCardSpec({
    required this.category,
    required this.requiredFields,
    required this.helpHint,
  });

  final EvidenceCategory category;
  final List<EvidenceField> requiredFields;
  final String helpHint;

  static const List<EvidenceCardSpec> all = [
    EvidenceCardSpec(
      category: EvidenceCategory.documentaryContract,
      requiredFields: [
        EvidenceField(key: 'doc_type', label: '文书类型'),
        EvidenceField(key: 'signed_at', label: '签署日期'),
      ],
      helpHint: '劳动合同、工资条、解除通知等书面材料。',
    ),
    EvidenceCardSpec(
      category: EvidenceCategory.audioVideo,
      requiredFields: [
        EvidenceField(key: 'recorded_at', label: '录制时间'),
        EvidenceField(key: 'participants', label: '参与人'),
      ],
      helpHint: '录音 / 录像。请确保为本人参与的对话，未经剪辑。',
    ),
    EvidenceCardSpec(
      category: EvidenceCategory.digitalCommunication,
      requiredFields: [
        EvidenceField(key: 'platform', label: '平台'),
        EvidenceField(key: 'counterparty', label: '对方身份'),
      ],
      helpHint: '微信 / 短信 / 邮件等电子通讯记录。',
    ),
    EvidenceCardSpec(
      category: EvidenceCategory.witnessStatement,
      requiredFields: [
        EvidenceField(key: 'witness_name', label: '证人姓名', highSensitive: true),
        EvidenceField(key: 'relation', label: '与本人关系'),
      ],
      helpHint: '证人证言。证人个人信息属高敏，将本地加密处理。',
    ),
    EvidenceCardSpec(
      category: EvidenceCategory.scenePhotoVideo,
      requiredFields: [
        EvidenceField(key: 'location', label: '拍摄地点'),
        EvidenceField(key: 'taken_at', label: '拍摄时间'),
      ],
      helpHint: '现场照片 / 视频。建议附 GPS 与时间元数据。',
    ),
    EvidenceCardSpec(
      category: EvidenceCategory.thirdPartyData,
      requiredFields: [
        EvidenceField(key: 'source_org', label: '来源机构'),
        EvidenceField(key: 'account', label: '账户/单号', highSensitive: true),
      ],
      helpHint: '银行流水 / 税单 / 社保记录。账号属高敏，将本地加密。',
    ),
    EvidenceCardSpec(
      category: EvidenceCategory.appraisal,
      requiredFields: [
        EvidenceField(key: 'appraisal_org', label: '鉴定机构'),
        EvidenceField(key: 'conclusion', label: '鉴定结论'),
      ],
      helpHint: '工伤 / 职业病 / 伤残鉴定与评估报告。',
    ),
  ];
}

/// A single evidence card form widget (one of the seven). It collects the metadata + a file path
/// and reports the assembled metadata map back to the collector for `POST /case/:id/evidence`.
class EvidenceCardForm extends StatefulWidget {
  const EvidenceCardForm({super.key, required this.spec, required this.onChanged});
  final EvidenceCardSpec spec;
  final void Function(Map<String, dynamic> metadata) onChanged;

  @override
  State<EvidenceCardForm> createState() => _EvidenceCardFormState();
}

class _EvidenceCardFormState extends State<EvidenceCardForm> {
  final Map<String, TextEditingController> _controllers = {};

  @override
  void initState() {
    super.initState();
    for (final f in widget.spec.requiredFields) {
      _controllers[f.key] = TextEditingController()..addListener(_emit);
    }
  }

  @override
  void dispose() {
    for (final c in _controllers.values) {
      c.dispose();
    }
    super.dispose();
  }

  void _emit() {
    widget.onChanged({
      'category': widget.spec.category.wire,
      for (final f in widget.spec.requiredFields) f.key: _controllers[f.key]!.text,
    });
  }

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(widget.spec.category.labelZh,
                style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 4),
            Text(widget.spec.helpHint, style: Theme.of(context).textTheme.bodySmall),
            const SizedBox(height: 8),
            ...widget.spec.requiredFields.map((f) => Padding(
                  padding: const EdgeInsets.only(bottom: 8),
                  child: TextField(
                    controller: _controllers[f.key],
                    decoration: InputDecoration(
                      labelText: f.label + (f.highSensitive ? ' · 高敏' : ''),
                      labelStyle: f.highSensitive ? TextStyle(color: colors.sealRed) : null,
                      border: const OutlineInputBorder(),
                      isDense: true,
                    ),
                  ),
                )),
          ],
        ),
      ),
    );
  }
}
