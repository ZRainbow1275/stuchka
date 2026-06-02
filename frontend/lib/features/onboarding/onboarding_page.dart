import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../theme/stuchka_icons.dart';
import '../../theme/stuchka_theme.dart';
import 'onboarding_state.dart';
import 'onboarding_validator.dart';

/// First-run wizard (ai/01 §1.6 / brief §5). Five steps, each a card; a `required` step must pass
/// [OnboardingValidator.validate] before "下一步". `primary != secondary` is enforced.
class OnboardingPage extends ConsumerStatefulWidget {
  const OnboardingPage({super.key, required this.onComplete});
  final VoidCallback onComplete;

  @override
  ConsumerState<OnboardingPage> createState() => _OnboardingPageState();
}

class _OnboardingPageState extends ConsumerState<OnboardingPage> {
  int _index = 0;
  String? _stepError;
  final _signature = TextEditingController();

  static const _channels = ['deepseek', 'qwen-cloud', 'local-qwen', 'overseas-gpt'];

  OnboardingStep get _step => OnboardingStep.values[_index];

  @override
  void dispose() {
    _signature.dispose();
    super.dispose();
  }

  void _next() {
    final ctrl = ref.read(onboardingControllerProvider.notifier);
    final err = ctrl.validate(_step);
    if (err != null) {
      setState(() => _stepError = err.messageZh);
      return;
    }
    setState(() => _stepError = null);
    if (_index < OnboardingStep.values.length - 1) {
      setState(() => _index += 1);
    } else if (ctrl.isComplete) {
      widget.onComplete();
    } else {
      setState(() => _stepError = '请完成所有必填步骤');
    }
  }

  void _back() {
    if (_index > 0) setState(() => _index -= 1);
  }

  @override
  Widget build(BuildContext context) {
    final draft = ref.watch(onboardingControllerProvider);
    final ctrl = ref.read(onboardingControllerProvider.notifier);
    final colors = StuchkaSemanticColors.of(context);

    return Scaffold(
      appBar: AppBar(title: Text('首次设置 · ${_index + 1}/${OnboardingStep.values.length}')),
      body: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 560),
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(_step.title, style: Theme.of(context).textTheme.titleLarge),
                const SizedBox(height: 16),
                _buildStep(draft, ctrl, colors),
                if (_stepError != null) ...[
                  const SizedBox(height: 12),
                  Text(_stepError!, style: TextStyle(color: colors.riskHigh)),
                ],
                const SizedBox(height: 24),
                Row(
                  children: [
                    if (_index > 0)
                      TextButton(onPressed: _back, child: const Text('上一步')),
                    const Spacer(),
                    FilledButton.icon(
                      onPressed: _next,
                      icon: const Icon(StuchkaIcons.chevronRight),
                      label: Text(_index == OnboardingStep.values.length - 1 ? '完成' : '下一步'),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _buildStep(OnboardingDraft draft, OnboardingController ctrl, StuchkaSemanticColors colors) {
    switch (_step) {
      case OnboardingStep.pickPrimary:
        return _ChannelPicker(
          value: draft.primary,
          options: _channels,
          onPick: ctrl.setPrimary,
        );
      case OnboardingStep.pickSecondary:
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text('备用通道不能与主用通道相同。'),
            const SizedBox(height: 8),
            _ChannelPicker(
              value: draft.secondary,
              options: _channels,
              disabled: draft.primary,
              onPick: ctrl.setSecondary,
            ),
          ],
        );
      case OnboardingStep.overseasOptIn:
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            SwitchListTile(
              value: draft.overseasEnabled,
              title: const Text('启用境外云（默认关闭）'),
              activeThumbColor: colors.sealRed,
              onChanged: (v) => ctrl.setOverseas(v, signature: _signature.text),
            ),
            if (draft.overseasEnabled) ...[
              const SizedBox(height: 8),
              const Text('请手写签名："我同意数据出境"（不可记忆，每次确认）'),
              const SizedBox(height: 6),
              TextField(
                controller: _signature,
                decoration: const InputDecoration(border: OutlineInputBorder(), isDense: true),
                onChanged: (v) => ctrl.setOverseas(true, signature: v),
              ),
            ],
          ],
        );
      case OnboardingStep.downloadLocalQwen:
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text('本地 Qwen 模型约 5GB，可选。下载后离线也能使用本地 AI。'),
            const SizedBox(height: 8),
            OutlinedButton.icon(
              icon: Icon(draft.localQwenDownloaded ? StuchkaIcons.check : StuchkaIcons.download),
              label: Text(draft.localQwenDownloaded ? '已下载' : '模拟下载'),
              onPressed: () => ctrl.setLocalQwen(!draft.localQwenDownloaded),
            ),
          ],
        );
      case OnboardingStep.kbInitialSync:
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text('知识库初始化为必需步骤（阻塞）。'),
            const SizedBox(height: 8),
            OutlinedButton.icon(
              icon: Icon(draft.kbSynced ? StuchkaIcons.check : StuchkaIcons.refresh),
              label: Text(draft.kbSynced ? '已初始化' : '开始初始化'),
              onPressed: () => ctrl.setKbSynced(true),
            ),
          ],
        );
    }
  }
}

class _ChannelPicker extends StatelessWidget {
  const _ChannelPicker({required this.value, required this.options, required this.onPick, this.disabled});
  final String? value;
  final List<String> options;
  final String? disabled;
  final void Function(String) onPick;

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: options.map((o) {
        final isDisabled = o == disabled;
        return ChoiceChip(
          selected: value == o,
          label: Text(o),
          onSelected: isDisabled ? null : (_) => onPick(o),
        );
      }).toList(),
    );
  }
}
