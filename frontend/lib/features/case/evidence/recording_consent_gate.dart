import 'package:flutter/material.dart';

import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';

/// 录音 / 录像证据合法性确认门 (法律 P5).
///
/// 我国民事诉讼中，私录的视听资料并非一律不可采：只有以**严重侵害他人合法权益、违反法律禁止性规定，
/// 或严重违背公序良俗**的方法取得的，才会被排除（《最高人民法院关于民事诉讼证据的若干规定》）。本门
/// 在用户导入录音 / 录像证据前，要求其就取得方式作一次知情确认，并把确认结果写入证据元数据，供后续
/// 人工复核。它**不**替用户判断证据是否合法（系统不作合法性承诺），只如实记录用户声明 —— 不勾选即不导入。
class RecordingConsentGate {
  RecordingConsentGate._();

  /// Returns true iff the user affirmatively acknowledged the legality statement. Returns false on
  /// cancel / dismiss (the caller must then abort the import — no fabricated consent).
  static Future<bool> show(BuildContext context) async {
    final result = await showDialog<bool>(
      context: context,
      barrierDismissible: false,
      builder: (ctx) => const _RecordingConsentDialog(),
    );
    return result ?? false;
  }
}

class _RecordingConsentDialog extends StatefulWidget {
  const _RecordingConsentDialog();

  @override
  State<_RecordingConsentDialog> createState() => _RecordingConsentDialogState();
}

class _RecordingConsentDialogState extends State<_RecordingConsentDialog> {
  bool _checked = false;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    return AlertDialog(
      icon: Icon(StuchkaIcons.alert, color: colors.riskMid),
      title: const Text('录音 / 录像合法性确认'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            '私自录制的录音 / 录像可以作为证据，但若以严重侵害他人合法权益、违反法律禁止性规定，'
            '或严重违背公序良俗的方法取得（例如在他人私密空间安装窃听 / 偷拍设备），则不被采纳。',
          ),
          const SizedBox(height: 12),
          Text(
            '请确认本材料的取得方式合法，再导入。系统据实记录你的声明，但不对其合法性作出承诺，'
            '最终采纳与否由仲裁庭 / 法院依法认定。',
            style: Theme.of(context).textTheme.bodySmall?.copyWith(color: colors.inkBlue),
          ),
          const SizedBox(height: 12),
          CheckboxListTile(
            contentPadding: EdgeInsets.zero,
            controlAffinity: ListTileControlAffinity.leading,
            value: _checked,
            onChanged: (v) => setState(() => _checked = v ?? false),
            title: const Text('我确认：该录音 / 录像系合法取得，未采用上述被禁止的手段。'),
          ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(false),
          child: const Text('取消'),
        ),
        FilledButton(
          onPressed: _checked ? () => Navigator.of(context).pop(true) : null,
          child: const Text('确认并导入'),
        ),
      ],
    );
  }
}
