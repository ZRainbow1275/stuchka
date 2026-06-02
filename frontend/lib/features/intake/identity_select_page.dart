import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:intl/intl.dart';

import '../../ipc/dto/dtos.dart';
import '../../ipc/dto/enums.dart';
import '../../ipc/rust_core_client.dart';
import '../../ipc/rust_core_providers.dart';
import '../../theme/stuchka_icons.dart';
import '../../theme/stuchka_theme.dart';
import '../case/case_providers.dart';

/// /intake/identity — 11-choice identity grid + region/time + dispute subtype → `POST /case`.
class IdentitySelectPage extends ConsumerStatefulWidget {
  const IdentitySelectPage({super.key});

  @override
  ConsumerState<IdentitySelectPage> createState() => _IdentitySelectPageState();
}

class _IdentitySelectPageState extends ConsumerState<IdentitySelectPage> {
  IdentityType? _identity;
  DisputeSubtype _subtype = DisputeSubtype.socialInsArrears;
  final _province = TextEditingController(text: '广东省');
  final _city = TextEditingController(text: '深圳市');
  final _firstDesc = TextEditingController();
  DateTime _occurredAt = DateTime.now();
  bool _submitting = false;
  String? _error;

  @override
  void dispose() {
    _province.dispose();
    _city.dispose();
    _firstDesc.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (_identity == null) {
      setState(() => _error = '请先选择您的用工身份（11 选 1）');
      return;
    }
    setState(() {
      _submitting = true;
      _error = null;
    });
    final client = ref.read(rustCoreClientProvider);
    try {
      // kbVersionHash must be the live hash (INV-04 server freeze ignores stale client values).
      String kbHash = '';
      try {
        kbHash = (await client.kbVersion()).versionHash;
      } catch (_) {}
      final req = CreateCaseReq(
        identityType: _identity!,
        province: _province.text.trim(),
        city: _city.text.trim(),
        caseOccurredAt: DateFormat('yyyy-MM-dd').format(_occurredAt),
        disputeSubtype: _subtype,
        firstDescription: _firstDesc.text.trim(),
        kbVersionHash: kbHash,
      );
      final created = await client.createCase(req);
      ref.invalidate(caseListProvider);
      if (mounted) context.go('/intake/diagnose?caseId=${created.id}');
    } on RustCoreApiException catch (e) {
      setState(() => _error = '${e.code}: ${e.message}');
    } catch (e) {
      setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _submitting = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    return Scaffold(
      appBar: AppBar(title: const Text('身份分流 · 11 选 1')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Text('请选择最贴近您的用工身份', style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 12),
          GridView.count(
            crossAxisCount: 3,
            shrinkWrap: true,
            physics: const NeverScrollableScrollPhysics(),
            mainAxisSpacing: 8,
            crossAxisSpacing: 8,
            childAspectRatio: 2.6,
            children: IdentityType.values.map((id) {
              final selected = _identity == id;
              return Material(
                color: selected ? colors.inkBlue : colors.rice,
                borderRadius: BorderRadius.circular(8),
                child: InkWell(
                  borderRadius: BorderRadius.circular(8),
                  onTap: () => setState(() => _identity = id),
                  child: Padding(
                    padding: const EdgeInsets.all(8),
                    child: Center(
                      child: Text(
                        id.labelZh,
                        textAlign: TextAlign.center,
                        style: TextStyle(
                          color: selected ? colors.paper : colors.char,
                          fontWeight: selected ? FontWeight.w700 : FontWeight.w400,
                        ),
                      ),
                    ),
                  ),
                ),
              );
            }).toList(),
          ),
          const SizedBox(height: 20),
          Text('争议子类型', style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 4),
          // NOTE: backend data-model currently models only the 8 social-insurance subtypes; the
          // 农民工欠薪 (WAGE_ARREARS) main path is a placeholder until data-model extends the enum.
          DropdownButtonFormField<DisputeSubtype>(
            initialValue: _subtype,
            items: DisputeSubtype.values
                .map((s) => DropdownMenuItem(value: s, child: Text(s.labelZh)))
                .toList(),
            onChanged: (v) => setState(() => _subtype = v ?? _subtype),
          ),
          const SizedBox(height: 16),
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _province,
                  decoration: const InputDecoration(labelText: '省', border: OutlineInputBorder()),
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: TextField(
                  controller: _city,
                  decoration: const InputDecoration(labelText: '市', border: OutlineInputBorder()),
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              const Icon(StuchkaIcons.deadline, size: 18),
              const SizedBox(width: 8),
              Text('事发日期：${DateFormat('yyyy-MM-dd').format(_occurredAt)}'),
              const Spacer(),
              TextButton(
                onPressed: () async {
                  final picked = await showDatePicker(
                    context: context,
                    initialDate: _occurredAt,
                    firstDate: DateTime(2015),
                    lastDate: DateTime.now(),
                  );
                  if (picked != null) setState(() => _occurredAt = picked);
                },
                child: const Text('选择日期'),
              ),
            ],
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _firstDesc,
            maxLines: 3,
            decoration: const InputDecoration(
              labelText: '简要描述（首次陈述）',
              border: OutlineInputBorder(),
            ),
          ),
          if (_error != null) ...[
            const SizedBox(height: 12),
            Text(_error!, style: TextStyle(color: colors.riskHigh)),
          ],
          const SizedBox(height: 20),
          FilledButton.icon(
            onPressed: _submitting ? null : _submit,
            icon: _submitting
                ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))
                : const Icon(StuchkaIcons.check),
            label: const Text('创建案件并进入诊断'),
          ),
        ],
      ),
    );
  }
}
