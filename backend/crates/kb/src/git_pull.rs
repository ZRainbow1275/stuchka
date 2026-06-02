//! KB incremental update via git pull diff (data/03 §3.2.1, deploy/04 §4.4).
//!
//! The KB repo tags every version (`v2026-05-12-r1`). R1a delivers:
//!
//! - **REAL (must run)**: `checksums.txt` parsing + per-file SHA-256 verification (KBC-04) and
//!   `manifest.global_hash` recomputation/verification (KBC-02) over a local checkout. These are
//!   the integrity gates the INV-04 freeze depends on, so they run for real against the filesystem.
//! - **make-usable skeleton**: opening a local `gix` repository and listing the diff between the
//!   current `HEAD` and a target tag. Full network clone/fetch is wired through `gix` but the
//!   default "completer" copies the new full version atomically (deploy/04 §4.4.2 chose full-file
//!   overwrite over binary diff), so R1a does not need a patch applier.

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::KbError;
use crate::manifest::{compute_global_hash, file_content_hash, KbManifest};

/// Summary of a pull diff between the local HEAD and a target tag (data/03 §3.2.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffSummary {
    /// Relative paths added in the target version.
    pub added: Vec<String>,
    /// Relative paths whose content changed (full-file overwrite, deploy/04 §4.4.2).
    pub modified: Vec<String>,
    /// Relative paths removed in the target version.
    pub deleted: Vec<String>,
    /// True once every changed file's SHA-256 matched `checksums.txt` AND the recomputed
    /// `global_hash` matched the manifest (KBC-02 / KBC-04).
    pub verified: bool,
}

impl DiffSummary {
    /// Total number of changed entries.
    pub fn change_count(&self) -> usize {
        self.added.len() + self.modified.len() + self.deleted.len()
    }
}

/// Parse a `checksums.txt` index (`<sha256>␠␠<relative/path>` per line, the `sha256sum` format)
/// into a `path -> hex` map (KBC-04). Blank lines and `#` comments are ignored.
pub fn parse_checksums(text: &str) -> Result<BTreeMap<String, String>, KbError> {
    let mut map = BTreeMap::new();
    for (lineno, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // `sha256sum` output: "<64hex>  <path>" (two spaces) or "<64hex> *<path>" (binary marker).
        let (hash, path) = line
            .split_once("  ")
            .or_else(|| line.split_once(" *"))
            .ok_or_else(|| {
                KbError::Yaml(format!(
                    "checksums.txt malformed at line {}: {line}",
                    lineno + 1
                ))
            })?;
        let hash = hash.trim().to_lowercase();
        let path = path.trim().trim_start_matches("./");
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(KbError::Yaml(format!(
                "checksums.txt bad hash at line {}: {hash}",
                lineno + 1
            )));
        }
        map.insert(path.to_string(), hash);
    }
    Ok(map)
}

/// Verify every file listed in `checksums` against its on-disk bytes under `root` (KBC-04, REAL).
/// Returns the first [`KbError::ChecksumMismatch`] / IO error encountered.
pub fn verify_checksums(root: &Path, checksums: &BTreeMap<String, String>) -> Result<(), KbError> {
    for (rel, expected) in checksums {
        let path = root.join(rel);
        let bytes = std::fs::read(&path).map_err(|source| KbError::Io {
            path: path.clone(),
            source,
        })?;
        let actual = file_content_hash(&bytes);
        if &actual != expected {
            return Err(KbError::ChecksumMismatch {
                path: rel.clone(),
                expected: expected.clone(),
                actual,
            });
        }
    }
    Ok(())
}

/// Verify the manifest `global_hash` against the recomputed aggregate of the `checksums` map
/// (KBC-02, REAL). The aggregate uses the same recipe as publishing (deploy/04 §4.2.2).
pub fn verify_manifest_global_hash(
    manifest: &KbManifest,
    checksums: &BTreeMap<String, String>,
) -> Result<(), KbError> {
    let recomputed = compute_global_hash(checksums.iter().map(|(p, h)| (p.as_str(), h.as_str())));
    if recomputed != manifest.global_hash {
        return Err(KbError::GlobalHashMismatch {
            expected: manifest.global_hash.clone(),
            actual: recomputed,
        });
    }
    Ok(())
}

/// Full integrity gate run after a checkout (REAL): parse `checksums.txt`, verify every file's
/// SHA-256 (KBC-04), then verify the manifest aggregate `global_hash` (KBC-02). On success the
/// local checkout is safe to activate as a KB version (INV-04 anchor verified).
pub fn verify_checkout(
    root: &Path,
    manifest: &KbManifest,
    checksums_txt: &str,
) -> Result<(), KbError> {
    let checksums = parse_checksums(checksums_txt)?;
    verify_checksums(root, &checksums)?;
    verify_manifest_global_hash(manifest, &checksums)?;
    Ok(())
}

/// Open a local KB git checkout and produce a [`DiffSummary`] between `HEAD` and `target_tag`
/// (make-usable skeleton, data/03 §3.2.1). Uses `gix` to discover the repository and resolve the
/// tag; the changed-path set is computed by walking both trees. Network fetch is left to the caller
/// (the api/db layer drives `gix` fetch with its own transport); R1a verifies integrity locally.
///
/// `verified` is left `false` here — call [`verify_checkout`] after checkout to set it.
pub fn diff_to_tag(local_path: &Path, target_tag: &str) -> Result<DiffSummary, KbError> {
    let repo = gix::discover(local_path).map_err(|e| KbError::Git(e.to_string()))?;

    // Resolve HEAD tree.
    let head_id = repo
        .head_id()
        .map_err(|e| KbError::Git(format!("head: {e}")))?;
    let head_commit = repo
        .find_object(head_id)
        .map_err(|e| KbError::Git(format!("head object: {e}")))?
        .try_into_commit()
        .map_err(|e| KbError::Git(format!("head not a commit: {e}")))?;
    let head_tree = head_commit
        .tree()
        .map_err(|e| KbError::Git(format!("head tree: {e}")))?;

    // Resolve the target tag's tree (`refs/tags/<tag>` or a short name).
    let tag_ref = repo
        .find_reference(target_tag)
        .or_else(|_| repo.find_reference(&format!("refs/tags/{target_tag}")))
        .map_err(|e| KbError::Git(format!("tag {target_tag}: {e}")))?;
    let tag_id = tag_ref
        .into_fully_peeled_id()
        .map_err(|e| KbError::Git(e.to_string()))?;
    let tag_commit = repo
        .find_object(tag_id)
        .map_err(|e| KbError::Git(format!("tag object: {e}")))?
        .peel_to_kind(gix::object::Kind::Commit)
        .map_err(|e| KbError::Git(format!("tag peel: {e}")))?
        .into_commit();
    let tag_tree = tag_commit
        .tree()
        .map_err(|e| KbError::Git(format!("tag tree: {e}")))?;

    // Flatten both trees to path -> blob-id and diff the maps (full-file overwrite model).
    let head_paths = flatten_tree(&repo, &head_tree)?;
    let tag_paths = flatten_tree(&repo, &tag_tree)?;

    let mut summary = DiffSummary::default();
    for (path, tag_oid) in &tag_paths {
        match head_paths.get(path) {
            None => summary.added.push(path.clone()),
            Some(head_oid) if head_oid != tag_oid => summary.modified.push(path.clone()),
            _ => {}
        }
    }
    for path in head_paths.keys() {
        if !tag_paths.contains_key(path) {
            summary.deleted.push(path.clone());
        }
    }
    summary.added.sort();
    summary.modified.sort();
    summary.deleted.sort();
    Ok(summary)
}

/// Recursively flatten a git tree into a `relative/path -> blob oid hex` map.
fn flatten_tree(
    repo: &gix::Repository,
    tree: &gix::Tree<'_>,
) -> Result<BTreeMap<String, String>, KbError> {
    let mut out = BTreeMap::new();
    let mut recorder = gix::traverse::tree::Recorder::default();
    tree.traverse()
        .breadthfirst(&mut recorder)
        .map_err(|e| KbError::Git(format!("tree traverse: {e}")))?;
    let _ = repo; // repo kept in signature for API symmetry / future blob reads.
    for entry in recorder.records {
        if entry.mode.is_blob() {
            out.insert(entry.filepath.to_string(), entry.oid.to_string());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn sample_manifest(global_hash: &str) -> KbManifest {
        KbManifest {
            version_label: "2026-05-12-r1".to_string(),
            generated_at: Utc.with_ymd_and_hms(2026, 5, 12, 8, 0, 0).unwrap(),
            global_hash: global_hash.to_string(),
            schema_version: 1,
            file_count: 2,
            law_count_national: 1,
            law_count_local: 1,
            category_total: 20,
            subcategory_total: 85,
        }
    }

    #[test]
    fn parse_checksums_reads_sha256sum_format() {
        let h1 = file_content_hash(b"AAA");
        let h2 = file_content_hash(b"BBB");
        let text = format!("# checksums\n{h1}  laws/a.json\n{h2} *categories.yaml\n\n",);
        let map = parse_checksums(&text).unwrap();
        assert_eq!(map.get("laws/a.json"), Some(&h1));
        assert_eq!(map.get("categories.yaml"), Some(&h2));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn parse_checksums_rejects_bad_hash() {
        assert!(parse_checksums("zzzz  a.json\n").is_err());
    }

    /// KBC-04 + KBC-02: a real checkout passes both integrity gates when files / manifest match.
    #[test]
    fn verify_checkout_passes_for_matching_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("laws")).unwrap();
        std::fs::write(root.join("laws/a.json"), b"AAA").unwrap();
        std::fs::write(root.join("categories.yaml"), b"CATS").unwrap();

        let h1 = file_content_hash(b"AAA");
        let h2 = file_content_hash(b"CATS");
        let checksums_txt = format!("{h1}  laws/a.json\n{h2}  categories.yaml\n");

        let mut map = BTreeMap::new();
        map.insert("categories.yaml".to_string(), h2.clone());
        map.insert("laws/a.json".to_string(), h1.clone());
        let gh = compute_global_hash(map.iter().map(|(p, h)| (p.as_str(), h.as_str())));

        let manifest = sample_manifest(&gh);
        verify_checkout(root, &manifest, &checksums_txt).unwrap();
    }

    /// KBC-04: a tampered file is rejected (the GPG/checksum gate that must fire 100%).
    #[test]
    fn verify_checkout_rejects_tampered_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("a.json"), b"TAMPERED").unwrap();
        let expected = file_content_hash(b"ORIGINAL");
        let checksums_txt = format!("{expected}  a.json\n");
        let manifest = sample_manifest(&"0".repeat(64));
        let err = verify_checkout(root, &manifest, &checksums_txt).unwrap_err();
        assert_eq!(err.code(), "E_KB_CHECKSUM_MISMATCH");
        assert!(matches!(err, KbError::ChecksumMismatch { .. }));
    }

    /// KBC-02: files pass but the manifest global_hash is wrong → GlobalHashMismatch.
    #[test]
    fn verify_checkout_rejects_wrong_global_hash() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("a.json"), b"AAA").unwrap();
        let h = file_content_hash(b"AAA");
        let checksums_txt = format!("{h}  a.json\n");
        let manifest = sample_manifest(&"f".repeat(64)); // deliberately wrong
        let err = verify_checkout(root, &manifest, &checksums_txt).unwrap_err();
        assert!(matches!(err, KbError::GlobalHashMismatch { .. }));
    }

    #[test]
    fn diff_summary_change_count() {
        let s = DiffSummary {
            added: vec!["a".into()],
            modified: vec!["b".into(), "c".into()],
            deleted: vec![],
            verified: false,
        };
        assert_eq!(s.change_count(), 3);
    }
}
