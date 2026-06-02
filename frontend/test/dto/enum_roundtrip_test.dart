import 'package:flutter_test/flutter_test.dart';
import 'package:stuchka/ipc/dto/dtos.dart';
import 'package:stuchka/ipc/dto/enums.dart';

void main() {
  group('enum wire values match backend data-model snake_case', () {
    test('IdentityType — verbatim from enums.rs', () {
      expect(IdentityType.standardFullTime.wire, 'standard_full_time');
      expect(IdentityType.deFactoNoContract.wire, 'de_facto_no_contract');
      expect(IdentityType.constructionLabor.wire, 'construction_labor');
      expect(IdentityType.retiredRehired.wire, 'retired_rehired');
      // round-trip
      for (final v in IdentityType.values) {
        expect(IdentityType.fromWire(v.wire), v);
      }
    });

    test('DisputeSubtype — 8 social-insurance branches', () {
      expect(DisputeSubtype.socialInsWaiverInvalid.wire, 'social_ins_waiver_invalid');
      expect(DisputeSubtype.socialInsBaseDispute.wire, 'social_ins_base_dispute');
      expect(DisputeSubtype.values.length, 8);
    });

    test('EvidenceCategory — 7 classes', () {
      expect(EvidenceCategory.documentaryContract.wire, 'documentary_contract');
      expect(EvidenceCategory.thirdPartyData.wire, 'third_party_data');
      expect(EvidenceCategory.values.length, 7);
    });

    test('SourceTag uses online (not web) and inferred (not infer)', () {
      expect(SourceTag.online.wire, 'online');
      expect(SourceTag.inferred.wire, 'inferred');
      expect(SourceTag.fromWire('online'), SourceTag.online);
      expect(SourceTag.fromWire('inferred'), SourceTag.inferred);
    });

    test('CaseStatus / FactStatus / ClaimType / CoverageTag', () {
      expect(CaseStatus.frozen.wire, 'frozen');
      expect(FactStatus.deprecated.wire, 'deprecated');
      expect(ClaimType.maliciousArrearsSurcharge.wire, 'malicious_arrears_surcharge');
      expect(ClaimType.doubleWageNoContract.wire, 'double_wage_no_contract');
      expect(CoverageTag.unknown.wire, 'unknown');
    });

    test('AuditCategory / LawLevel', () {
      expect(AuditCategory.mergeDecision.wire, 'merge_decision');
      expect(LawLevel.judicialInterpretation.wire, 'judicial_interpretation');
    });
  });

  group('DTO fromJson against backend camelCase fixtures', () {
    test('CaseDto.fromJson (routes/case.rs::CaseDto)', () {
      final j = {
        'id': '0190a0c0-0000-7000-8000-000000000001',
        'status': 'draft',
        'identityType': 'construction_labor',
        'province': '广东省',
        'city': '深圳市',
        'regionCode': null,
        'caseOccurredAt': '2026-01-15',
        'disputeSubtype': 'social_ins_arrears',
        'disputeCategoryId': null,
        'coverageTier': 'make_usable',
        'coverageTag': null,
        'kbVersionHash': 'abc123',
        'createdAt': '2026-06-02T08:00:00Z',
        'updatedAt': '2026-06-02T08:00:00Z',
      };
      final c = CaseDto.fromJson(j);
      expect(c.identityType, IdentityType.constructionLabor);
      expect(c.status, CaseStatus.draft);
      expect(c.disputeSubtype, DisputeSubtype.socialInsArrears);
      expect(c.coverageTier, CoverageTier.makeUsable);
      expect(c.kbVersionHash, 'abc123');
      expect(c.createdAt, isNotNull);
    });

    test('FactDto.fromJson (routes/fact.rs::FactDto)', () {
      final j = {
        'id': 'f1',
        'caseId': 'c1',
        'content': '欠薪 3 个月',
        'category': 'wage',
        'status': 'pending',
        'source': 'user_input',
        'confidence': 0.42,
        'coverageTag': 'unknown',
        'createdAt': '2026-06-02T08:00:00Z',
        'updatedAt': '2026-06-02T08:00:00Z',
      };
      final f = FactDto.fromJson(j);
      expect(f.category, FactCategory.wage);
      expect(f.status, FactStatus.pending);
      expect(f.source, FactSource.userInput);
      expect(f.confidence, closeTo(0.42, 1e-9));
      expect(f.coverageTag, CoverageTag.unknown);
    });

    test('EvidenceDto.fromJson incl. five-dimension score breakdown', () {
      final j = {
        'id': 'e1',
        'caseId': 'c1',
        'category': 'audio_video',
        'status': 'scored',
        'filePath': '/data/evidence/e1',
        'fileSha256': 'deadbeef',
        'fileSizeBytes': 1024,
        'isHighSensitive': true,
        'piiHits': [],
        'effectiveScore': 0.73,
        'scoreBreakdown': {
          'source': 0.8,
          'temporal': 0.7,
          'integrity': 0.6,
          'relevance': 0.75,
          'authenticity': 0.7,
        },
        'linkedFactIds': ['f1'],
        'collectedAt': null,
        'createdAt': '2026-06-02T08:00:00Z',
      };
      final e = EvidenceDto.fromJson(j);
      expect(e.category, EvidenceCategory.audioVideo);
      expect(e.status, EvidenceStatus.scored);
      expect(e.isHighSensitive, isTrue);
      expect(e.effectiveScore, closeTo(0.73, 1e-9));
      expect(e.scoreBreakdown.source, closeTo(0.8, 1e-9));
      expect(e.linkedFactIds, ['f1']);
    });

    test('ApiEnvelope.fromJson unwraps data + traceId (camelCase)', () {
      final j = {
        'data': {'status': 'ok', 'version': '0.1.0'},
        'error': null,
        'traceId': 'trace-1',
      };
      final env = ApiEnvelope.fromJson<HealthDto>(
        j,
        (d) => HealthDto.fromJson(d as Map<String, dynamic>),
      );
      expect(env.isOk, isTrue);
      expect(env.traceId, 'trace-1');
      expect(env.data!.isReady, isTrue);
    });

    test('ApiError.fromJson maps the §1.12 code string', () {
      final j = {
        'data': null,
        'error': {
          'code': 'E_KB_OUTDATED',
          'message': '知识库已超过 30 天',
          'detail': null,
          'hint': null,
        },
        'traceId': 't',
      };
      final env = ApiEnvelope.fromJson<HealthDto>(
        j,
        (d) => HealthDto.fromJson(d as Map<String, dynamic>),
      );
      expect(env.isOk, isFalse);
      expect(env.error!.code, 'E_KB_OUTDATED');
    });
  });

  group('CreateCaseReq.toJson emits camelCase + snake_case enum values', () {
    test('matches CreateCaseReq wire shape', () {
      final req = CreateCaseReq(
        identityType: IdentityType.constructionLabor,
        province: '广东省',
        city: '深圳市',
        caseOccurredAt: '2026-01-15',
        disputeSubtype: DisputeSubtype.socialInsArrears,
        firstDescription: '欠薪',
        kbVersionHash: 'h',
      );
      final j = req.toJson();
      expect(j['identityType'], 'construction_labor');
      expect(j['disputeSubtype'], 'social_ins_arrears');
      expect(j['caseOccurredAt'], '2026-01-15');
      expect(j['kbVersionHash'], 'h');
    });
  });
}
