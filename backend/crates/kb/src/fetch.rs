//! KB distribution endpoint fallback (data/03 §3.2, deploy/04 §4.3.2).
//!
//! Resolution order: GitHub Pages → jsDelivr CDN → user mirror → local cache. R1a delivers the
//! endpoint-list / URL-resolution skeleton (make-usable): given a relative path it yields the full
//! candidate URLs in priority order so the client/db layer can drive the actual HTTP fetch with its
//! own runtime (reqwest). The crate itself stays transport-agnostic for R1a.

use crate::error::KbError;

/// Default distribution endpoints in priority order (deploy/04 §4.3.2). jsDelivr is tried first for
/// global acceleration, then GitHub Pages origin, then the gitee mirror.
pub const DEFAULT_ENDPOINTS: [&str; 3] = [
    "https://cdn.jsdelivr.net/gh/stuchka/stuchka-kb@main",
    "https://stuchka.github.io/stuchka-kb",
    "https://gitee.com/stuchka/stuchka-kb/raw/main",
];

/// The ordered set of distribution endpoints plus an optional user mirror override and a local
/// cache directory (the terminal fallback, data/03 §3.2).
#[derive(Debug, Clone)]
pub struct EndpointSet {
    endpoints: Vec<String>,
    local_cache: Option<std::path::PathBuf>,
}

impl Default for EndpointSet {
    fn default() -> Self {
        Self {
            endpoints: DEFAULT_ENDPOINTS.iter().map(|s| s.to_string()).collect(),
            local_cache: None,
        }
    }
}

/// A single resolved fetch candidate: either a remote URL or the local cache path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchCandidate {
    /// A fully-qualified remote URL (`<endpoint><path>`).
    Url(String),
    /// The terminal local-cache file path (continue-with-stale, triggers an expiry warning).
    LocalCache(std::path::PathBuf),
}

impl EndpointSet {
    /// Build an endpoint set, optionally prepending a user mirror override (highest priority, after
    /// the defaults are kept as further fallbacks) and setting a local cache root.
    pub fn new(mirror_override: Option<String>, local_cache: Option<std::path::PathBuf>) -> Self {
        let mut endpoints: Vec<String> = Vec::new();
        if let Some(mirror) = mirror_override {
            endpoints.push(mirror);
        }
        endpoints.extend(DEFAULT_ENDPOINTS.iter().map(|s| s.to_string()));
        Self {
            endpoints,
            local_cache,
        }
    }

    /// Resolve a relative repo path (e.g. `/manifest.json`) into the ordered candidate list:
    /// every endpoint URL first, then the local cache file as the terminal fallback (data/03 §3.2).
    pub fn resolve(&self, path: &str) -> Vec<FetchCandidate> {
        let rel = path.strip_prefix('/').unwrap_or(path);
        let mut out: Vec<FetchCandidate> = self
            .endpoints
            .iter()
            .map(|base| FetchCandidate::Url(format!("{}/{}", base.trim_end_matches('/'), rel)))
            .collect();
        if let Some(cache) = &self.local_cache {
            out.push(FetchCandidate::LocalCache(cache.join(rel)));
        }
        out
    }

    /// Drive a per-candidate fetch closure in priority order, returning the first success
    /// (make-usable: the caller supplies the actual transport so the crate has no HTTP runtime
    /// dependency in R1a). Returns [`KbError::AllEndpointsFailed`] if every candidate fails.
    pub fn fetch_with_fallback<T, F>(&self, path: &str, mut try_one: F) -> Result<T, KbError>
    where
        F: FnMut(&FetchCandidate) -> Option<T>,
    {
        for candidate in self.resolve(path) {
            if let Some(ok) = try_one(&candidate) {
                return Ok(ok);
            }
        }
        Err(KbError::AllEndpointsFailed(path.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_lists_endpoints_then_cache() {
        let set = EndpointSet::new(None, Some(std::path::PathBuf::from("/tmp/kb")));
        let cands = set.resolve("/manifest.json");
        // 3 default endpoints + 1 local cache
        assert_eq!(cands.len(), 4);
        assert_eq!(
            cands[0],
            FetchCandidate::Url(
                "https://cdn.jsdelivr.net/gh/stuchka/stuchka-kb@main/manifest.json".to_string()
            )
        );
        assert!(matches!(cands[3], FetchCandidate::LocalCache(_)));
    }

    #[test]
    fn mirror_override_takes_priority() {
        let set = EndpointSet::new(Some("https://mirror.example.cn/kb".to_string()), None);
        let cands = set.resolve("manifest.json");
        assert_eq!(
            cands[0],
            FetchCandidate::Url("https://mirror.example.cn/kb/manifest.json".to_string())
        );
        // defaults remain as further fallbacks
        assert_eq!(cands.len(), 4);
    }

    #[test]
    fn fetch_with_fallback_returns_first_success() {
        let set = EndpointSet::default();
        // Simulate the first two endpoints failing and the third succeeding.
        let mut attempts = 0;
        let got = set
            .fetch_with_fallback("/manifest.json", |c| {
                attempts += 1;
                if let FetchCandidate::Url(u) = c {
                    if u.contains("gitee") {
                        return Some(u.clone());
                    }
                }
                None
            })
            .unwrap();
        assert!(got.contains("gitee"));
        assert_eq!(attempts, 3);
    }

    #[test]
    fn fetch_with_fallback_errors_when_all_fail() {
        let set = EndpointSet::default();
        let err = set
            .fetch_with_fallback::<(), _>("/manifest.json", |_| None)
            .unwrap_err();
        assert_eq!(err.code(), "E_KB_FETCH_FAILED");
    }
}
