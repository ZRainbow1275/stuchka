//! Claim — arbitration request item with state machine (data/01 §1.5).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::enums::{ClaimStatus, ClaimType};
use crate::time::Timestamp;
use crate::traits::Identified;

/// Arbitration claim (data/01 §1.5.6). Each Claim links 1+ LawRef + 1+ CaseFact + 0+ Evidence;
/// amounts are computed by the M9 rule engine, never by AI (§4.6.4, INV-01).
///
/// Amounts are `rust_decimal::Decimal` (fixed-point); f32/f64 are forbidden for money.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Claim {
    pub id: Uuid,
    pub case_id: Uuid,
    pub claim_type: ClaimType,
    /// Pre-tax amount (M9 computed). Fixed-point; never f64.
    pub amount_pre_tax: Option<Decimal>,
    pub amount_post_tax: Option<Decimal>,
    /// Formula + inputs + law sources (references `law_ref.stable_id`).
    pub calculation_breakdown: serde_json::Value,
    pub status: ClaimStatus,
    /// Soft references to `law_ref.id`.
    pub law_refs: Vec<Uuid>,
    /// Soft references to `fact.id`.
    pub fact_refs: Vec<Uuid>,
    /// INV-10 mandatory disclaimer confirmation for withdrawal.
    pub withdraw_inv10_confirmed: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Identified for Claim {
    fn id(&self) -> Uuid {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::new_id;
    use crate::time::now;
    use serde_json::json;
    use std::str::FromStr;

    fn sample(amount: Option<Decimal>) -> Claim {
        Claim {
            id: new_id(),
            case_id: new_id(),
            claim_type: ClaimType::EconomicCompensation,
            amount_pre_tax: amount,
            amount_post_tax: None,
            calculation_breakdown: json!({"formula": "N+1", "base": "8000.00"}),
            status: ClaimStatus::Draft,
            law_refs: vec![new_id()],
            fact_refs: vec![new_id()],
            withdraw_inv10_confirmed: false,
            created_at: now(),
            updated_at: now(),
        }
    }

    #[test]
    fn claim_serde_roundtrip() {
        let c = sample(Some(Decimal::from_str("24000.00").unwrap()));
        let j = serde_json::to_string(&c).unwrap();
        assert!(
            j.contains("\"claimType\":\"economic_compensation\""),
            "got {j}"
        );
        assert!(j.contains("\"withdrawInv10Confirmed\":false"), "got {j}");
        let back: Claim = serde_json::from_str(&j).unwrap();
        assert_eq!(back.amount_pre_tax, c.amount_pre_tax);
        assert_eq!(back.id(), c.id);
    }

    /// OM-05: amount is fixed-point Decimal and survives string round-trip without precision loss.
    #[test]
    fn decimal_amount_string_roundtrip_no_precision_loss() {
        for raw in [
            "0.00",
            "0.01",
            "8000.00",
            "123456789012.99",
            "24000.50",
            "999999999.999999999",
        ] {
            let d = Decimal::from_str(raw).unwrap();
            // round-trip via Decimal's own string form
            let s = d.to_string();
            let back = Decimal::from_str(&s).unwrap();
            assert_eq!(d, back, "Decimal string round-trip lost precision: {raw}");

            // round-trip embedded in the Claim JSON (serde keeps Decimal exact)
            let c = sample(Some(d));
            let j = serde_json::to_string(&c).unwrap();
            let back_claim: Claim = serde_json::from_str(&j).unwrap();
            assert_eq!(
                back_claim.amount_pre_tax,
                Some(d),
                "JSON round-trip lost precision: {raw}"
            );
        }
    }
}
