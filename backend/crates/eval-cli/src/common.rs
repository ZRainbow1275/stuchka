//! Shared helpers: dataset loading + coverage-tag stringification.

use std::path::Path;

use rule_engine::CoverageTag;
use serde::de::DeserializeOwned;

/// Read + deserialize a JSON dataset, mapping every failure to a human string (no panics).
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("cannot read dataset {}: {e}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|e| format!("cannot parse dataset {}: {e}", path.display()))
}

/// Stable lowercase string for a coverage tag (matches the dataset `coverage_tag` field).
pub fn coverage_tag_str(t: CoverageTag) -> &'static str {
    match t {
        CoverageTag::Exact => "exact",
        CoverageTag::Approximate => "approximate",
        CoverageTag::Boundary => "boundary",
        CoverageTag::Unknown => "unknown",
    }
}
