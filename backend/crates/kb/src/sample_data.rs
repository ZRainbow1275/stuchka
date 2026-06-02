//! Real sample law entries seeding the BM25 index for tests + dev (KB-01/02, KBC-02/03, LR-01..07).
//!
//! These are genuine PRC labour-law statutes (real article numbers, real URNs per data/02 §2.2)
//! covering the five R1 deep categories so BM25 search actually hits:
//!
//! - 欠薪 wage arrears (LD-03)            — 劳动合同法 §30, 调解仲裁法 §6, 工资支付暂行规定 §7
//! - 违法解除 illegal termination (LD-02) — 劳动合同法 §39 §40 §46 §47 §87
//! - 未签合同 no contract (LD-01)         — 劳动合同法 §10 §82
//! - 工伤 work injury (LD-05)             — 工伤保险条例 §14 §17 §62
//! - 加班费 overtime (LD-09)              — 劳动法 §44, 劳动合同法 §31, 调解仲裁法 §27
//!
//! The bodies are the substantive statutory text (abbreviated where long but verbatim in the
//! load-bearing portion) so `content_hash` is meaningful (KBC-04) and BM25 tokenisation has real
//! Chinese terms to match.

use serde::{Deserialize, Serialize};

use crate::error::KbError;

/// A seed law entry: its D8 URN, human title, dispute category code, region and the clause body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleLaw {
    pub stable_id: &'static str,
    pub title: &'static str,
    /// `dispute_category` LD-NN-NN this clause is most relevant to.
    pub category: &'static str,
    /// GB/T 2260 region code (`"00"` = national; `"44"` = Guangdong, data/02 §2.5).
    pub region_code: &'static str,
    pub body: &'static str,
}

/// 22 real labour-law clauses across the five R1 deep categories (data/02 §2.2 URN form).
pub const SAMPLE_LAWS: &[SampleLaw] = &[
    // ---- 未签合同 no contract (LD-01) ----
    SampleLaw {
        stable_id: "law:中华人民共和国劳动合同法/v2012-12-28/§10",
        title: "中华人民共和国劳动合同法",
        category: "LD-01-01",
        region_code: "00",
        body: "建立劳动关系，应当订立书面劳动合同。已建立劳动关系，未同时订立书面劳动合同的，应当自用工之日起一个月内订立书面劳动合同。",
    },
    SampleLaw {
        stable_id: "law:中华人民共和国劳动合同法/v2012-12-28/§82",
        title: "中华人民共和国劳动合同法",
        category: "LD-01-01",
        region_code: "00",
        body: "用人单位自用工之日起超过一个月不满一年未与劳动者订立书面劳动合同的，应当向劳动者每月支付二倍的工资。用人单位违反本法规定不与劳动者订立无固定期限劳动合同的，自应当订立无固定期限劳动合同之日起向劳动者每月支付二倍的工资。",
    },
    SampleLaw {
        stable_id: "law:中华人民共和国劳动合同法/v2012-12-28/§14",
        title: "中华人民共和国劳动合同法",
        category: "LD-01-02",
        region_code: "00",
        body: "无固定期限劳动合同，是指用人单位与劳动者约定无确定终止时间的劳动合同。用人单位与劳动者协商一致，可以订立无固定期限劳动合同。用人单位自用工之日起满一年不与劳动者订立书面劳动合同的，视为用人单位与劳动者已订立无固定期限劳动合同。",
    },
    // ---- 违法解除 illegal termination (LD-02) ----
    SampleLaw {
        stable_id: "law:中华人民共和国劳动合同法/v2012-12-28/§39/¶1",
        title: "中华人民共和国劳动合同法",
        category: "LD-02-06",
        region_code: "00",
        body: "劳动者有下列情形之一的，用人单位可以解除劳动合同：在试用期间被证明不符合录用条件的；严重违反用人单位的规章制度的；严重失职，营私舞弊，给用人单位造成重大损害的。",
    },
    SampleLaw {
        stable_id: "law:中华人民共和国劳动合同法/v2012-12-28/§40",
        title: "中华人民共和国劳动合同法",
        category: "LD-02-01",
        region_code: "00",
        body: "有下列情形之一的，用人单位提前三十日以书面形式通知劳动者本人或者额外支付劳动者一个月工资后，可以解除劳动合同：劳动者患病或者非因工负伤，在规定的医疗期满后不能从事原工作的；劳动者不能胜任工作，经过培训或者调整工作岗位，仍不能胜任工作的。",
    },
    SampleLaw {
        stable_id: "law:中华人民共和国劳动合同法/v2012-12-28/§46",
        title: "中华人民共和国劳动合同法",
        category: "LD-02-03",
        region_code: "00",
        body: "有下列情形之一的，用人单位应当向劳动者支付经济补偿：劳动者依照本法第三十八条规定解除劳动合同的；用人单位依照本法第四十条规定解除劳动合同的；用人单位依照本法第四十一条第一款规定解除劳动合同的。",
    },
    SampleLaw {
        stable_id: "law:中华人民共和国劳动合同法/v2012-12-28/§47",
        title: "中华人民共和国劳动合同法",
        category: "LD-02-03",
        region_code: "00",
        body: "经济补偿按劳动者在本单位工作的年限，每满一年支付一个月工资的标准向劳动者支付。六个月以上不满一年的，按一年计算；不满六个月的，向劳动者支付半个月工资的经济补偿。",
    },
    SampleLaw {
        stable_id: "law:中华人民共和国劳动合同法/v2012-12-28/§87",
        title: "中华人民共和国劳动合同法",
        category: "LD-02-01",
        region_code: "00",
        body: "用人单位违反本法规定解除或者终止劳动合同的，应当依照本法第四十七条规定的经济补偿标准的二倍向劳动者支付赔偿金。",
    },
    SampleLaw {
        stable_id: "law:中华人民共和国劳动合同法/v2012-12-28/§38",
        title: "中华人民共和国劳动合同法",
        category: "LD-02-04",
        region_code: "00",
        body: "用人单位有下列情形之一的，劳动者可以解除劳动合同：未按照劳动合同约定提供劳动保护或者劳动条件的；未及时足额支付劳动报酬的；未依法为劳动者缴纳社会保险费的。",
    },
    // ---- 工资报酬 / 欠薪 wage arrears (LD-03) ----
    SampleLaw {
        stable_id: "law:中华人民共和国劳动合同法/v2012-12-28/§30",
        title: "中华人民共和国劳动合同法",
        category: "LD-03-01",
        region_code: "00",
        body: "用人单位应当按照劳动合同约定和国家规定，向劳动者及时足额支付劳动报酬。用人单位拖欠或者未足额支付劳动报酬的，劳动者可以依法向当地人民法院申请支付令，人民法院应当依法发出支付令。",
    },
    SampleLaw {
        stable_id: "law:中华人民共和国劳动合同法/v2012-12-28/§85",
        title: "中华人民共和国劳动合同法",
        category: "LD-03-05",
        region_code: "00",
        body: "用人单位未按照劳动合同的约定或者国家规定及时足额支付劳动者劳动报酬的，由劳动行政部门责令限期支付；逾期不支付的，责令用人单位按应付金额百分之五十以上百分之一百以下的标准向劳动者加付赔偿金。",
    },
    SampleLaw {
        stable_id: "law:中华人民共和国劳动争议调解仲裁法/v2008-05-01/§6",
        title: "中华人民共和国劳动争议调解仲裁法",
        category: "LD-03-06",
        region_code: "00",
        body: "发生劳动争议，当事人对自己提出的主张，有责任提供证据。与争议事项有关的证据属于用人单位掌握管理的，用人单位应当提供；用人单位不提供的，应当承担不利后果。",
    },
    SampleLaw {
        stable_id: "law:工资支付暂行规定/v1994-12-06/§7",
        title: "工资支付暂行规定",
        category: "LD-03-01",
        region_code: "00",
        body: "工资必须在用人单位与劳动者约定的日期支付。如遇节假日或休息日，则应提前在最近的工作日支付。工资至少每月支付一次，实行周、日、小时工资制的可按周、日、小时支付工资。",
    },
    SampleLaw {
        stable_id: "law:广东省工资支付条例/v2016-09-29/§44",
        title: "广东省工资支付条例",
        category: "LD-03-01",
        region_code: "44",
        body: "用人单位克扣或者无故拖欠劳动者工资的，由县级以上人民政府人力资源社会保障部门责令限期支付；逾期不支付的，责令用人单位按应付金额百分之五十以上百分之一百以下的标准计算，向劳动者加付赔偿金。",
    },
    // ---- 工伤 work injury (LD-05) ----
    SampleLaw {
        stable_id: "law:工伤保险条例/v2010-12-20/§14/¶1/（一）",
        title: "工伤保险条例",
        category: "LD-05-01",
        region_code: "00",
        body: "职工有下列情形之一的，应当认定为工伤：在工作时间和工作场所内，因工作原因受到事故伤害的；工作时间前后在工作场所内，从事与工作有关的预备性或者收尾性工作受到事故伤害的。",
    },
    SampleLaw {
        stable_id: "law:工伤保险条例/v2010-12-20/§14/¶1/（六）",
        title: "工伤保险条例",
        category: "LD-05-01",
        region_code: "00",
        body: "在上下班途中，受到非本人主要责任的交通事故或者城市轨道交通、客运轮渡、火车事故伤害的，应当认定为工伤。",
    },
    SampleLaw {
        stable_id: "law:工伤保险条例/v2010-12-20/§17",
        title: "工伤保险条例",
        category: "LD-05-01",
        region_code: "00",
        body: "职工发生事故伤害，所在单位应当自事故伤害发生之日起三十日内，向统筹地区社会保险行政部门提出工伤认定申请。用人单位未按规定提出工伤认定申请的，工伤职工或者其近亲属可以在事故伤害发生之日起一年内直接提出工伤认定申请。",
    },
    SampleLaw {
        stable_id: "law:工伤保险条例/v2010-12-20/§62",
        title: "工伤保险条例",
        category: "LD-05-05",
        region_code: "00",
        body: "用人单位依照本条例规定应当参加工伤保险而未参加的，由社会保险行政部门责令限期参加，并自欠缴之日起，按日加收万分之五的滞纳金。职工发生工伤时，用人单位未参加工伤保险的，由该用人单位按照本条例规定的工伤保险待遇项目和标准支付费用。",
    },
    // ---- 加班费 / 休息休假 overtime (LD-09) ----
    SampleLaw {
        stable_id: "law:中华人民共和国劳动法/v1995-01-01/§44",
        title: "中华人民共和国劳动法",
        category: "LD-09-01",
        region_code: "00",
        body: "有下列情形之一的，用人单位应当按照下列标准支付高于劳动者正常工作时间工资的工资报酬：安排延长工作时间的，支付不低于工资百分之一百五十的工资报酬；休息日安排工作又不能安排补休的，支付不低于工资百分之二百的工资报酬；法定休假日安排工作的，支付不低于工资百分之三百的工资报酬。",
    },
    SampleLaw {
        stable_id: "law:中华人民共和国劳动法/v1995-01-01/§41",
        title: "中华人民共和国劳动法",
        category: "LD-09-02",
        region_code: "00",
        body: "用人单位由于生产经营需要，经与工会和劳动者协商后可以延长工作时间，一般每日不得超过一小时；因特殊原因需要延长工作时间的，在保障劳动者身体健康的条件下延长工作时间每日不得超过三小时，但是每月不得超过三十六小时。",
    },
    SampleLaw {
        stable_id: "law:中华人民共和国劳动合同法/v2012-12-28/§31",
        title: "中华人民共和国劳动合同法",
        category: "LD-09-01",
        region_code: "00",
        body: "用人单位应当严格执行劳动定额标准，不得强迫或者变相强迫劳动者加班。用人单位安排加班的，应当按照国家有关规定向劳动者支付加班费。",
    },
    SampleLaw {
        stable_id: "law:中华人民共和国劳动争议调解仲裁法/v2008-05-01/§27",
        title: "中华人民共和国劳动争议调解仲裁法",
        category: "LD-09-01",
        region_code: "00",
        body: "劳动争议申请仲裁的时效期间为一年。仲裁时效期间从当事人知道或者应当知道其权利被侵害之日起计算。劳动关系存续期间因拖欠劳动报酬发生争议的，劳动者申请仲裁不受本条第一款规定的仲裁时效期间的限制。",
    },
];

/// Build the per-file content-hash map for the sample corpus (one virtual file per clause), so a
/// `manifest.global_hash` can be computed deterministically over the seed set (KBC-02 wiring).
pub fn sample_content_hashes() -> std::collections::BTreeMap<String, String> {
    SAMPLE_LAWS
        .iter()
        .map(|l| (l.stable_id.to_string(), data_model::content_hash(l.body)))
        .collect()
}

/// Validate every sample URN parses (LR-04..07 over real data) and return the parsed count.
pub fn validate_sample_urns() -> Result<usize, KbError> {
    for law in SAMPLE_LAWS {
        data_model::parse_law_ref(law.stable_id)?;
    }
    Ok(SAMPLE_LAWS.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn at_least_20_sample_laws() {
        assert!(
            SAMPLE_LAWS.len() >= 20,
            "need >= 20 seed laws, got {}",
            SAMPLE_LAWS.len()
        );
    }

    /// All five R1 deep categories are represented in the seed corpus.
    #[test]
    fn covers_five_deep_categories() {
        let prefixes: BTreeSet<&str> = SAMPLE_LAWS
            .iter()
            .map(|l| &l.category[..5]) // LD-NN
            .collect();
        for want in ["LD-01", "LD-02", "LD-03", "LD-05", "LD-09"] {
            assert!(prefixes.contains(want), "missing deep category {want}");
        }
    }

    /// LR-04..07 over real data: every seed URN parses via the data-model D8 parser.
    #[test]
    fn every_sample_urn_parses() {
        assert_eq!(validate_sample_urns().unwrap(), SAMPLE_LAWS.len());
    }

    /// Every category code is a valid LD-NN-NN.
    #[test]
    fn every_category_code_is_well_formed() {
        for l in SAMPLE_LAWS {
            let b = l.category.as_bytes();
            assert_eq!(b.len(), 8, "bad category {}", l.category);
            assert_eq!(&b[0..3], b"LD-");
        }
    }

    /// KBC-04: every clause body hashes to a 64-hex content_hash via data-model.
    #[test]
    fn sample_content_hashes_are_64_hex() {
        let map = sample_content_hashes();
        assert_eq!(map.len(), SAMPLE_LAWS.len());
        for h in map.values() {
            assert_eq!(h.len(), 64);
            assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }
}
