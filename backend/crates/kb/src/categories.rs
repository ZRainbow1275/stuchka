//! `categories.yaml` — 20 categories / 85 subcategories (data/01 §1.7, data/03 KBC-03).
//!
//! Parsed with `serde_norway` (the maintained YAML crate; `serde_yaml` is deprecated, W6) and
//! double-checked: total counts (20 / 85, KBC-03) and every `dispute_category` code matching
//! `^LD-[0-9]{2}-[0-9]{2}$` (data/01 §1.7). A bundled default `categories.yaml` ships with the
//! crate so the count assertion runs at build time and against a known-good baseline.

use serde::{Deserialize, Serialize};

use crate::error::KbError;

/// The canonical `categories.yaml` bundled with the crate (20 categories / 85 subcategories).
pub const DEFAULT_CATEGORIES_YAML: &str = include_str!("../assets/categories.yaml");

/// A 20-category labour-dispute taxonomy parsed from `categories.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryCatalog {
    pub categories: Vec<Category>,
}

/// One top-level dispute category (`LD-NN`) and its subcategories.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Category {
    /// Top-level code, e.g. `LD-01`.
    pub code: String,
    /// Chinese category name.
    pub name: String,
    pub subcategories: Vec<Subcategory>,
}

/// One dispute subcategory (`LD-NN-NN`, matches `case.dispute_category`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subcategory {
    /// Full `dispute_category` code, e.g. `LD-01-01` (matches `^LD-[0-9]{2}-[0-9]{2}$`).
    pub code: String,
    pub name: String,
}

impl CategoryCatalog {
    /// Parse a `categories.yaml` document and run the KBC-03 count + code-format assertions.
    pub fn parse(yaml: &str) -> Result<Self, KbError> {
        let catalog: CategoryCatalog =
            serde_norway::from_str(yaml).map_err(|e| KbError::Yaml(e.to_string()))?;
        catalog.validate()?;
        Ok(catalog)
    }

    /// Load the bundled default catalog (always valid; used as the baseline / fallback).
    pub fn bundled() -> Result<Self, KbError> {
        Self::parse(DEFAULT_CATEGORIES_YAML)
    }

    /// Number of top-level categories (must be 20).
    pub fn category_count(&self) -> usize {
        self.categories.len()
    }

    /// Total subcategories across all categories (must be 85).
    pub fn subcategory_count(&self) -> usize {
        self.categories.iter().map(|c| c.subcategories.len()).sum()
    }

    /// KBC-03 assertion: exactly 20 categories / 85 subcategories, every subcategory code matching
    /// `^LD-[0-9]{2}-[0-9]{2}$` and every category code matching `^LD-[0-9]{2}$`.
    pub fn validate(&self) -> Result<(), KbError> {
        let categories = self.category_count();
        let subcategories = self.subcategory_count();
        if categories != 20 || subcategories != 85 {
            return Err(KbError::CategoryCountInvalid {
                categories,
                subcategories,
            });
        }
        for cat in &self.categories {
            if !is_category_code(&cat.code) {
                return Err(KbError::InvalidCategoryCode(cat.code.clone()));
            }
            for sub in &cat.subcategories {
                if !is_subcategory_code(&sub.code) {
                    return Err(KbError::InvalidCategoryCode(sub.code.clone()));
                }
            }
        }
        Ok(())
    }
}

/// `^LD-[0-9]{2}$` — a top-level category code.
fn is_category_code(code: &str) -> bool {
    let bytes = code.as_bytes();
    bytes.len() == 5
        && &bytes[0..3] == b"LD-"
        && bytes[3].is_ascii_digit()
        && bytes[4].is_ascii_digit()
}

/// `^LD-[0-9]{2}-[0-9]{2}$` — a subcategory / `dispute_category` code (data/01 §1.7).
fn is_subcategory_code(code: &str) -> bool {
    let bytes = code.as_bytes();
    bytes.len() == 8
        && &bytes[0..3] == b"LD-"
        && bytes[3].is_ascii_digit()
        && bytes[4].is_ascii_digit()
        && bytes[5] == b'-'
        && bytes[6].is_ascii_digit()
        && bytes[7].is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// KBC-03: the bundled categories.yaml is exactly 20 categories / 85 subcategories.
    #[test]
    fn kbc03_bundled_catalog_is_20_85() {
        let catalog = CategoryCatalog::bundled().expect("bundled categories.yaml must be valid");
        assert_eq!(catalog.category_count(), 20, "must have 20 categories");
        assert_eq!(
            catalog.subcategory_count(),
            85,
            "must have 85 subcategories"
        );
    }

    /// KBC-03: every code matches the LD-NN / LD-NN-NN format (data/01 §1.7).
    #[test]
    fn kbc03_all_codes_match_ld_format() {
        let catalog = CategoryCatalog::bundled().unwrap();
        catalog.validate().unwrap();
        for cat in &catalog.categories {
            assert!(
                is_category_code(&cat.code),
                "bad category code {}",
                cat.code
            );
            for sub in &cat.subcategories {
                assert!(
                    is_subcategory_code(&sub.code),
                    "bad subcategory code {}",
                    sub.code
                );
            }
        }
    }

    /// All category codes (LD-01..LD-20) and subcategory codes are unique.
    #[test]
    fn codes_are_unique() {
        let catalog = CategoryCatalog::bundled().unwrap();
        let mut seen = std::collections::HashSet::new();
        for cat in &catalog.categories {
            assert!(seen.insert(cat.code.clone()), "dup category {}", cat.code);
            for sub in &cat.subcategories {
                assert!(
                    seen.insert(sub.code.clone()),
                    "dup subcategory {}",
                    sub.code
                );
            }
        }
    }

    #[test]
    fn rejects_short_count() {
        let yaml = "categories:\n  - code: LD-01\n    name: x\n    subcategories:\n      - {code: LD-01-01, name: y}\n";
        let err = CategoryCatalog::parse(yaml).unwrap_err();
        assert!(matches!(err, KbError::CategoryCountInvalid { .. }));
    }

    #[test]
    fn code_format_checks() {
        assert!(is_category_code("LD-01"));
        assert!(!is_category_code("LD-1"));
        assert!(!is_category_code("LD-01-01"));
        assert!(is_subcategory_code("LD-20-05"));
        assert!(!is_subcategory_code("LD-20-5"));
        assert!(!is_subcategory_code("XX-20-05"));
    }
}
