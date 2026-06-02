import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:stuchka/features/case/document/document_host_page.dart';
import 'package:stuchka/features/case/flow/migrant_wage_scenario.dart';
import 'package:stuchka/features/case/flow/scenario_shell_page.dart';
import 'package:stuchka/theme/stuchka_theme.dart';

/// C6: the 农民工欠薪 scenario (ScenarioProgressBar) is wired into go_router and a real user can
/// navigate it (spec frontend/04 §4.9). Destination routes carry the §4.9 preset query params.
///
/// Mirrors the production route shape (`/scenario/migrant-wage/:step`, `/case/:id/documents`) in a
/// minimal router so the test exercises real navigation without the rust-core-dependent home/case
/// pages (those are covered elsewhere).
void main() {
  GoRouter buildRouter() => GoRouter(
        initialLocation: '/scenario/migrant-wage/identity',
        routes: [
          GoRoute(
            path: '/scenario/migrant-wage/:step',
            builder: (context, state) => ScenarioShellPage(
              stepId: state.pathParameters['step']!,
              caseId: state.uri.queryParameters['caseId'] ?? '',
            ),
          ),
          GoRoute(
            path: '/case/:id/documents',
            builder: (context, state) => DocumentHostPage(
              caseId: state.pathParameters['id']!,
              templates: (state.uri.queryParameters['templates'] ?? '')
                  .split(',')
                  .where((s) => s.isNotEmpty)
                  .toList(),
              exportRequested: state.uri.queryParameters['action'] == 'export',
            ),
          ),
        ],
      );

  testWidgets('a real user navigates /scenario/migrant-wage/:step and sees the progress bar',
      (tester) async {
    final router = buildRouter();
    await tester.pumpWidget(
      ProviderScope(child: MaterialApp.router(theme: StuchkaTheme.light(), routerConfig: router)),
    );
    await tester.pumpAndSettle();

    // Lands on the shell at step 1 with the pinned ScenarioProgressBar.
    expect(find.byType(ScenarioShellPage), findsOneWidget);
    expect(find.byType(ScenarioProgressBar), findsOneWidget);
    expect(find.textContaining('第 1 / 8 步'), findsWidgets);

    // Navigate to the evidence step (step 3) — destination honours the §4.9 focus preset.
    router.go('/scenario/migrant-wage/evidence?caseId=case-1');
    await tester.pumpAndSettle();
    expect(find.textContaining('第 3 / 8 步'), findsWidgets);
    expect(find.textContaining('focus=written,chat,third_party'), findsOneWidget);
  });

  testWidgets('the export step destination honours templates + action=export query params',
      (tester) async {
    final router = buildRouter();
    await tester.pumpWidget(
      ProviderScope(child: MaterialApp.router(theme: StuchkaTheme.light(), routerConfig: router)),
    );
    await tester.pumpAndSettle();

    // The documents step route from the flow (templates preset).
    router.go('/case/case-1/documents?templates=tpl_inspection,tpl_arbitration');
    await tester.pumpAndSettle();
    expect(find.byType(DocumentHostPage), findsOneWidget);
    expect(find.textContaining('tpl_inspection'), findsOneWidget);

    // The export action variant.
    router.go('/case/case-1/documents?action=export');
    await tester.pumpAndSettle();
    expect(find.textContaining('导出刑事报案材料（待确认）'), findsOneWidget);
  });

  test('every migrant-wage flow step id is a valid :step value', () {
    const flow = MigrantWageScenarioFlow();
    for (final s in flow.steps) {
      expect(s.id, isNotEmpty);
    }
    expect(flow.total, 8);
  });
}
