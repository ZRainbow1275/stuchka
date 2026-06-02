//! R1a regex strong-signal acceptance suite (`ai/04` §4.9: ≥ 50 unit cases).
//!
//! Covers brief acceptance assertions A1-A11, A15-A16, A21-A23 with ≥ 50 positive/negative cases
//! across phone / id card (ISO 7064) / bank card (Luhn) / audio path / medical path, plus span
//! back-mapping and performance budgets. All offline, via `HsdDetector::new_regex_only()`.

use hsd::{HsdDetector, PiiKind, RouteHint};

fn det() -> HsdDetector {
    HsdDetector::new_regex_only()
}

fn has_kind(text: &str, kind: PiiKind) -> bool {
    det().scan(text).hits.iter().any(|h| h.kind == kind)
}

// ---------------------------------------------------------------------------
// Phone (A1, A2) — positives
// ---------------------------------------------------------------------------

#[test]
fn phone_positives() {
    let cases = [
        "13800138000",
        "我老板的电话是13800138000，他从来不接我。",
        "联系15912345678",
        "号码：18612345678结束",
        "17012345678",
        "13312345678",
        "19912345678",
    ];
    for c in cases {
        assert!(has_kind(c, PiiKind::Phone), "phone expected in {c:?}");
    }
}

#[test]
fn a2_phone_with_spaces_and_prefix() {
    // normalize folds spaces; "+86 138 0013 8000" → phone detected.
    assert!(has_kind("+86 138 0013 8000", PiiKind::Phone));
    assert!(has_kind("１３８００１３８０００", PiiKind::Phone)); // fullwidth digits
}

#[test]
fn phone_negatives() {
    let cases = [
        "12345678901",     // starts with 12, not 1[3-9]
        "1380013800",      // 10 digits
        "138001380000",    // 12 digits (boundary rejects)
        "10086",           // hotline
        "请拨打95588咨询", // bank hotline
    ];
    for c in cases {
        assert!(!has_kind(c, PiiKind::Phone), "phone NOT expected in {c:?}");
    }
}

// ---------------------------------------------------------------------------
// Id card (A3, A4) — ISO 7064 mod 11-2
// ---------------------------------------------------------------------------

#[test]
fn idcard_positives() {
    // All verified ISO 7064 mod 11-2 valid.
    let cases = [
        "110101199001010074",
        "11010119900101004X",
        "身份证号110101199001010074请核对",
    ];
    for c in cases {
        assert!(has_kind(c, PiiKind::IdCard), "idcard expected in {c:?}");
    }
}

#[test]
fn idcard_negatives() {
    let cases = [
        "110101199001010070", // wrong check digit
        "110101199013010074", // month 13 invalid
        "110101199001320074", // day 32 invalid
        "010101199001010074", // leading 0 area
        "11010119900101007",  // 17 digits
    ];
    for c in cases {
        assert!(
            !has_kind(c, PiiKind::IdCard),
            "idcard NOT expected in {c:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Bank card (A5, A6) — Luhn
// ---------------------------------------------------------------------------

#[test]
fn bankcard_positives() {
    let cases = [
        "6222021234567894",    // 16-digit UnionPay, Luhn-valid
        "6225881234567890120", // 19-digit UnionPay, Luhn-valid
        "4000001234567899",    // Visa 16
        "5100001234567895",    // Mastercard 16
        "卡号6222021234567894打款",
    ];
    for c in cases {
        assert!(has_kind(c, PiiKind::BankCard), "bankcard expected in {c:?}");
    }
}

#[test]
fn a6_bankcard_negatives() {
    let cases = [
        "1234567890123456",  // 16-digit order number, not Luhn
        "6222021234567890",  // 62-prefix but Luhn fails
        "8888888888888888",  // unknown prefix
        "订单号62220212345", // too short
    ];
    for c in cases {
        assert!(
            !has_kind(c, PiiKind::BankCard),
            "bankcard NOT expected in {c:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Audio path (A7) / medical path (A8)
// ---------------------------------------------------------------------------

#[test]
fn audio_path_positives() {
    let cases = [
        "D:/录音/2024-03-01工资沟通.m4a",
        "~/录音/与张经理通话.mp3",
        "C:\\录音\\加班录像.wav",
        "../通话/老板.amr",
    ];
    for c in cases {
        assert!(
            has_kind(c, PiiKind::AudioPath),
            "audio path expected in {c:?}"
        );
    }
}

#[test]
fn medical_path_positives() {
    let cases = [
        "D:/病历/2024-03-15住院.pdf",
        "~/诊断书/工伤鉴定.jpg",
        "C:\\出院/记录.png",
        "../病历/检查.docx",
    ];
    for c in cases {
        assert!(
            has_kind(c, PiiKind::MedicalRecord),
            "medical path expected in {c:?}"
        );
    }
}

#[test]
fn path_negatives() {
    let cases = [
        "D:/文档/合同.pdf", // no medical keyword
        "D:/音乐/歌曲.mp3", // no audio keyword
        "录音很重要",       // keyword but no path/extension
    ];
    for c in cases {
        assert!(!has_kind(c, PiiKind::AudioPath), "no audio path in {c:?}");
        assert!(
            !has_kind(c, PiiKind::MedicalRecord),
            "no medical path in {c:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Decision / routing (A9, A10)
// ---------------------------------------------------------------------------

#[test]
fn a9_strong_signal_force_local() {
    for c in [
        "13800138000",
        "110101199001010074",
        "6222021234567894",
        "D:/录音/通话.mp3",
        "D:/病历/住院.pdf",
    ] {
        let rep = det().scan(c);
        assert!(rep.is_high_sensitive, "{c:?} must be high-sensitive");
        assert_eq!(rep.route_hint, RouteHint::ForceLocal, "{c:?}");
    }
}

#[test]
fn a10_weak_only_not_force_local() {
    // A single email (weight 0.6) → WarnAndConfirm, not high-sensitive.
    let rep = det().scan("联系邮箱 worker@example.com 即可");
    assert!(!rep.is_high_sensitive);
    assert_ne!(rep.route_hint, RouteHint::ForceLocal);
}

// ---------------------------------------------------------------------------
// Span back-mapping (A11)
// ---------------------------------------------------------------------------

#[test]
fn a11_span_maps_back_to_original() {
    let text = "我老板的电话是13800138000，他从来不接我。";
    let rep = det().scan(text);
    let phone = rep
        .hits
        .iter()
        .find(|h| h.kind == PiiKind::Phone)
        .expect("phone hit");
    assert_eq!(&text[phone.span.start..phone.span.end], "13800138000");
}

#[test]
fn a11_span_maps_back_with_fullwidth_and_spaces() {
    let text = "电话 +86 138 0013 8000 谢谢";
    let rep = det().scan(text);
    let phone = rep
        .hits
        .iter()
        .find(|h| h.kind == PiiKind::Phone)
        .expect("phone hit");
    // Original slice contains the spaced digits exactly.
    assert_eq!(&text[phone.span.start..phone.span.end], "138 0013 8000");
}

// ---------------------------------------------------------------------------
// Performance (A15, A16)
// ---------------------------------------------------------------------------

#[test]
fn a15_a16_performance_budget() {
    use std::time::Instant;
    // Build a ~1KB Chinese text with one phone embedded.
    let filler = "这是一段用于性能测试的中文文本内容。".repeat(40);
    let text = format!("{filler}联系电话13800138000{filler}");
    let det = det();

    // Warm up (LazyLock compile).
    let _ = det.scan(&text);

    let mut samples = Vec::with_capacity(200);
    for _ in 0..200 {
        let t = Instant::now();
        let rep = det.scan(&text);
        samples.push(t.elapsed());
        assert!(rep.is_high_sensitive);
    }
    samples.sort();
    let p95 = samples[(samples.len() as f64 * 0.95) as usize];
    assert!(
        p95.as_millis() < 200,
        "P95 scan latency {p95:?} must be < 200ms (A16)"
    );
}

// ---------------------------------------------------------------------------
// Mask (A21)
// ---------------------------------------------------------------------------

#[test]
fn a21_mask_renders_desensitized() {
    use hsd::mask;
    let text = "13800138000";
    let rep = det().scan(text);
    assert_eq!(mask(text, &rep.hits), "138****8000");

    let id = "110101199001010074";
    let rep = det().scan(id);
    assert_eq!(mask(id, &rep.hits), "110101********0074");
}

// ---------------------------------------------------------------------------
// API DTO compatibility (A22)
// ---------------------------------------------------------------------------

#[test]
fn a22_serialize_as_data_model_pii_hits() {
    let rep = det().scan("13800138000");
    let json = serde_json::to_value(&rep.hits).unwrap();
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["kind"], "phone");
    assert_eq!(arr[0]["ruleId"], "PII-PHONE-CN");
    assert!(arr[0]["span"]["start"].is_number());
}
