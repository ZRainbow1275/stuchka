//! Deep-coverage metadata (ai/02 §2.5). The five deep provinces are 11/31/32/33/44
//! (Beijing / Shanghai / Jiangsu / Zhejiang / Guangdong); the other 26 are make-usable.

/// The five deep-coverage province codes (GB/T 2260 first two digits).
pub const DEEP_PROVINCE_CODES: [&str; 5] = ["11", "31", "32", "33", "44"];

/// All 31 mainland province codes the engine embeds a YAML for.
pub const ALL_PROVINCE_CODES: [&str; 31] = [
    "11", "12", "13", "14", "15", // 京津冀晋蒙
    "21", "22", "23", // 辽吉黑
    "31", "32", "33", "34", "35", "36", "37", // 沪苏浙皖闽赣鲁
    "41", "42", "43", "44", "45", "46", // 豫鄂湘粤桂琼
    "50", "51", "52", "53", "54", // 渝川黔滇藏
    "61", "62", "63", "64", "65", // 陕甘青宁新
];

/// True for the five deep-coverage provinces.
pub fn is_deep(province_code: &str) -> bool {
    DEEP_PROVINCE_CODES.contains(&province_code)
}
