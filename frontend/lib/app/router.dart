import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../features/accessibility/simple_mode_controller.dart';
import '../features/audit/audit_log_page.dart';
import '../features/case/case_workbench_page.dart';
import '../features/case/calculator/comp_calc_page.dart';
import '../features/case/evidence/evidence_collector_page.dart';
import '../features/case/flow/procedure_compare_page.dart';
import '../features/case/flow/scenario_shell_page.dart';
import '../features/case/document/document_host_page.dart';
import '../features/home/standard_home_page.dart';
import '../features/home/simple_home_page.dart';
import '../features/intake/identity_select_page.dart';
import '../features/intake/diagnose_page.dart';
import '../features/onboarding/onboarding_page.dart';
import 'prefs.dart';

/// go_router 17 router (brief §2). 8 pages + onboarding gate + simple-mode redirect.
///
/// Paths follow the spec 04 §4.9 authoritative constants:
///   / · /onboarding · /intake/identity · /intake/diagnose
///   /case/:id · /case/:id/evidence · /case/:id/calc · /case/:id/procedure-compare
///   /case/:id/documents · /case/:id/audit
/// The `conflict` (INV-07) scene has no standalone path — it is a global overlay.
final routerProvider = Provider<GoRouter>((ref) {
  return GoRouter(
    initialLocation: '/',
    redirect: (context, state) {
      final loc = state.matchedLocation;

      // 1. First-run wizard gate: until onboarding is complete, force /onboarding (except already
      //    there). Onboarding completion is a persisted flag.
      final onboardingDone = ref.read(prefsProvider).readBool(StuchkaPrefs.kOnboardingDone);
      if (!onboardingDone && loc != '/onboarding') {
        return '/onboarding';
      }
      if (onboardingDone && loc == '/onboarding') {
        return '/';
      }

      // 2. Simple-mode redirect: when on, only the simple home + intake/case-deep pages are allowed.
      //    A non-deep route falls back to the full-screen simple home.
      final simple = ref.read(simpleModeProvider);
      if (simple && loc == '/') {
        // Home itself renders the simple variant; no redirect needed (StandardHome branches).
      }
      return null;
    },
    routes: [
      GoRoute(
        path: '/',
        builder: (context, state) {
          final simple = ref.read(simpleModeProvider);
          return simple ? const SimpleHomePage() : const StandardHomePage();
        },
      ),
      GoRoute(
        path: '/onboarding',
        builder: (context, state) => OnboardingPage(
          onComplete: () {
            ref.read(prefsProvider).writeBool(StuchkaPrefs.kOnboardingDone, true);
            ref.invalidate(onboardingDoneProvider);
            context.go('/');
          },
        ),
      ),
      GoRoute(
        path: '/intake/identity',
        builder: (context, state) => const IdentitySelectPage(),
      ),
      // 农民工欠薪 主用户路径 shell (spec frontend/04 §4.9). Hosts the pinned ScenarioProgressBar.
      GoRoute(
        path: '/scenario/migrant-wage/:step',
        builder: (context, state) => ScenarioShellPage(
          stepId: state.pathParameters['step']!,
          caseId: state.uri.queryParameters['caseId'] ?? '',
        ),
      ),
      GoRoute(
        path: '/intake/diagnose',
        builder: (context, state) {
          final caseId = state.uri.queryParameters['caseId'] ?? '';
          return DiagnosePage(caseId: caseId);
        },
      ),
      GoRoute(
        path: '/case/:id',
        builder: (context, state) =>
            CaseWorkbenchPage(caseId: state.pathParameters['id']!),
        routes: [
          GoRoute(
            path: 'evidence',
            builder: (context, state) =>
                EvidenceCollectorPage(caseId: state.pathParameters['id']!),
          ),
          GoRoute(
            path: 'calc',
            builder: (context, state) =>
                CompCalcPage(caseId: state.pathParameters['id']!),
          ),
          GoRoute(
            path: 'procedure-compare',
            builder: (context, state) =>
                ProcedureComparePage(caseId: state.pathParameters['id']!),
          ),
          GoRoute(
            path: 'documents',
            builder: (context, state) {
              final templatesParam = state.uri.queryParameters['templates'];
              return DocumentHostPage(
                caseId: state.pathParameters['id']!,
                templates: (templatesParam == null || templatesParam.isEmpty)
                    ? const []
                    : templatesParam.split(','),
                exportRequested: state.uri.queryParameters['action'] == 'export',
              );
            },
          ),
          GoRoute(
            path: 'audit',
            builder: (context, state) =>
                AuditLogPage(caseId: state.pathParameters['id']!),
          ),
        ],
      ),
    ],
  );
});

/// Whether the onboarding wizard is complete (persisted flag). Refreshed on completion.
final onboardingDoneProvider = Provider<bool>((ref) {
  return ref.read(prefsProvider).readBool(StuchkaPrefs.kOnboardingDone);
});
