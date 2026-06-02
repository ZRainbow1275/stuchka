import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/features/case/flow/migrant_wage_scenario.dart';

/// 主用户路径 · 农民工欠薪场景 (spec frontend/04 §4.9 / §4.11 R1a 端到端 全实装).
///
/// Locks the 8-step contract, the back/next navigation helpers, the skippable policy (the diagnosis
/// anchor steps are NOT skippable), and that each step resolves to a real `/case/:id/...` route.
void main() {
  const flow = MigrantWageScenarioFlow();
  const caseId = '019e86cd-9d8d-7e60-a567-f0ee92339a80';

  test('has the 8 spec §4.9 steps in order', () {
    expect(flow.total, 8);
    expect(
      flow.steps.map((s) => s.id).toList(),
      <String>[
        'identity',
        'diagnose',
        'evidence',
        'calc',
        'deadline',
        'procedure',
        'documents',
        'export',
      ],
    );
  });

  test('the diagnosis anchor steps are not skippable (spec §4.1.4)', () {
    final identity = flow.steps.firstWhere((s) => s.id == 'identity');
    final diagnose = flow.steps.firstWhere((s) => s.id == 'diagnose');
    expect(identity.skippable, isFalse);
    expect(diagnose.skippable, isFalse);
    // The collection steps are skippable (with the §4.9 confirmation).
    expect(flow.steps.firstWhere((s) => s.id == 'evidence').skippable, isTrue);
  });

  test('step numbers are 1-based and contiguous', () {
    expect(flow.stepNumber('identity'), 1);
    expect(flow.stepNumber('export'), 8);
    expect(flow.stepNumber('not_a_step'), -1);
  });

  test('next/previous walk the ordered path', () {
    expect(flow.next('identity')?.id, 'diagnose');
    expect(flow.next('export'), isNull);
    expect(flow.previous('identity'), isNull);
    expect(flow.previous('calc')?.id, 'evidence');
  });

  test('case steps resolve to real /case/:id/... routes with presets', () {
    final routes = {for (final s in flow.steps) s.id: s.routeOf(caseId)};
    expect(routes['identity'], '/intake/identity?preset=migrant_worker');
    expect(routes['diagnose'], contains('subtype=wage_arrears'));
    expect(routes['evidence'], '/case/$caseId/evidence?focus=written,chat,third_party');
    expect(routes['calc'], '/case/$caseId/calc?preset=wage_arrears_50pct');
    expect(routes['procedure'], '/case/$caseId/procedure-compare');
    expect(
      routes['documents'],
      '/case/$caseId/documents?templates=tpl_inspection,tpl_arbitration',
    );
    // Every case-scoped route is under the real router prefix.
    for (final id in ['evidence', 'calc', 'deadline', 'procedure', 'documents', 'export']) {
      expect(routes[id], startsWith('/case/$caseId'));
    }
  });
}
