//! Runtime configuration (D1 · `crates/core`).
//!
//! Reads `config/secrets.toml` (best-effort) and environment overrides into a single
//! [`RuntimeConfig`]. The backend binds `127.0.0.1:0` (INV-05: never `0.0.0.0`) and injects
//! `NO_PROXY=127.0.0.1,localhost` so that any outbound HTTP client (LLM providers, KB fetch)
//! does not route loopback IPC through the user's system proxy.
//!
//! The config file is intentionally minimal here: the first-run wizard (`first_run.rs`) and
//! the per-module subtasks fill in the substantive secrets schema. This module guarantees the
//! invariants that the IPC layer depends on (loopback host, NO_PROXY).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Loopback host the backend always binds to (INV-05; never `0.0.0.0`).
pub const LOOPBACK_HOST: &str = "127.0.0.1";

/// `NO_PROXY` value injected so loopback IPC bypasses the system proxy.
pub const NO_PROXY_VALUE: &str = "127.0.0.1,localhost";

/// Runtime configuration assembled from `config/secrets.toml` + environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RuntimeConfig {
    /// Bind host — always loopback (INV-05).
    #[serde(default = "default_host")]
    pub host: String,
    /// Requested port; `0` asks the OS for a random free port (the D1 default).
    #[serde(default)]
    pub port: u16,
    /// Directory holding the main store, audit.sqlite, and encrypted blobs.
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    /// Whether the first-run wizard still needs to run (no master key configured yet).
    #[serde(default)]
    pub first_run: bool,
    /// Master password used to derive the session KEK on a headless / dev boot. When absent the
    /// boot path generates-and-persists an `EncryptedDek` under the data dir via the real
    /// Argon2/HKDF path (never a fake key). Sourced from the `STUCHKA_MASTER_PASSWORD` env var or
    /// the optional `master_password` config key; NEVER logged or written to a tracked file.
    #[serde(default, skip_serializing)]
    pub master_password: Option<String>,
}

impl RuntimeConfig {
    /// Path to the main store (`<data_dir>/stuchka.sqlite`).
    pub fn main_db_path(&self) -> PathBuf {
        self.data_dir.join("stuchka.sqlite")
    }

    /// Path to the independent audit store (`<data_dir>/audit.sqlite`, D3 physical isolation).
    pub fn audit_db_path(&self) -> PathBuf {
        self.data_dir.join("audit.sqlite")
    }

    /// Path to the wrapped-DEK keystore (`<data_dir>/keystore/dek.enc`).
    pub fn keystore_path(&self) -> PathBuf {
        self.data_dir.join("keystore").join("dek.enc")
    }

    /// Directory holding age-encrypted high-sensitivity evidence blobs (`<data_dir>/blobs`).
    pub fn blob_dir(&self) -> PathBuf {
        self.data_dir.join("blobs")
    }

    /// Directory holding raw (non-high-sensitive) evidence files (`<data_dir>/evidence`).
    pub fn evidence_dir(&self) -> PathBuf {
        self.data_dir.join("evidence")
    }
}

fn default_host() -> String {
    LOOPBACK_HOST.to_string()
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("data")
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: 0,
            data_dir: default_data_dir(),
            first_run: true,
            master_password: None,
        }
    }
}

impl RuntimeConfig {
    /// Load configuration: start from defaults, overlay `config/secrets.toml` if present,
    /// then overlay environment variables. Injects `NO_PROXY` as a side effect.
    ///
    /// Missing or unreadable config file is not an error — defaults apply (the first-run
    /// wizard then writes the file).
    pub fn load() -> Self {
        let mut cfg = Self::from_file_or_default(Path::new("config/secrets.toml"));
        cfg.apply_env();
        cfg.normalize();
        inject_no_proxy();
        cfg
    }

    /// Parse a `config/secrets.toml`; on any error fall back to defaults.
    pub fn from_file_or_default(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(raw) => Self::from_toml_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Parse from a TOML string. Tolerant: a missing `[runtime]` table yields defaults.
    pub fn from_toml_str(raw: &str) -> Result<Self, ConfigError> {
        // Minimal TOML reader without a toml crate dependency: scan flat `key = value`
        // lines (optionally under a `[runtime]` table). Only the keys this module owns are
        // honored; everything else is ignored so the substantive secrets schema can grow
        // independently in later subtasks.
        let mut cfg = Self::default();
        let mut in_runtime = true; // top-level keys count as runtime
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') {
                in_runtime = line == "[runtime]";
                continue;
            }
            if !in_runtime {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = unquote(value.trim());
            match key {
                "host" => cfg.host = value.to_string(),
                "port" => {
                    cfg.port = value
                        .parse()
                        .map_err(|_| ConfigError::BadValue { key: "port" })?;
                }
                "data_dir" => cfg.data_dir = PathBuf::from(value),
                "first_run" => cfg.first_run = value == "true",
                "master_password" if !value.is_empty() => {
                    cfg.master_password = Some(value.to_string());
                }
                _ => {}
            }
        }
        Ok(cfg)
    }

    /// Overlay environment variables: `STUCHKA_HOST`, `STUCHKA_PORT`, `STUCHKA_DATA_DIR`.
    pub fn apply_env(&mut self) {
        if let Ok(h) = std::env::var("STUCHKA_HOST") {
            if !h.is_empty() {
                self.host = h;
            }
        }
        if let Ok(p) = std::env::var("STUCHKA_PORT") {
            if let Ok(p) = p.parse() {
                self.port = p;
            }
        }
        if let Ok(d) = std::env::var("STUCHKA_DATA_DIR") {
            if !d.is_empty() {
                self.data_dir = PathBuf::from(d);
            }
        }
        // Master password override (never logged; takes precedence over the config file).
        if let Ok(p) = std::env::var("STUCHKA_MASTER_PASSWORD") {
            if !p.is_empty() {
                self.master_password = Some(p);
            }
        }
    }

    /// Enforce INV-05: the bind host is always loopback regardless of file/env input.
    pub fn normalize(&mut self) {
        if self.host != LOOPBACK_HOST && self.host != "localhost" {
            self.host = LOOPBACK_HOST.to_string();
        }
    }

    /// `host:port` bind address string consumed by the TcpListener.
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// Inject `NO_PROXY=127.0.0.1,localhost` if not already covering loopback, so loopback IPC
/// and any localhost-bound services bypass the system proxy.
pub fn inject_no_proxy() {
    set_no_proxy_var("NO_PROXY");
    set_no_proxy_var("no_proxy");
}

fn set_no_proxy_var(name: &str) {
    match std::env::var(name) {
        Ok(existing) if existing.contains("127.0.0.1") => {}
        Ok(existing) if existing.is_empty() => std::env::set_var(name, NO_PROXY_VALUE),
        Ok(existing) => std::env::set_var(name, format!("{existing},{NO_PROXY_VALUE}")),
        Err(_) => std::env::set_var(name, NO_PROXY_VALUE),
    }
}

fn unquote(s: &str) -> &str {
    s.trim_matches('"').trim_matches('\'')
}

/// Configuration parse errors.
///
/// `Display` / `Error` are hand-written (not derived) because this crate is named `core`, which
/// shadows std `core` inside rustdoc's doctest pass and breaks macros that emit `core::*` paths.
#[derive(Debug)]
pub enum ConfigError {
    /// A known key held a value that could not be parsed.
    BadValue {
        /// The offending key name.
        key: &'static str,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::BadValue { key } => {
                write!(f, "config key `{key}` has an invalid value")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_loopback_random_port() {
        let cfg = RuntimeConfig::default();
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 0, "port 0 => OS-assigned random port (D1)");
        assert!(cfg.first_run);
    }

    #[test]
    fn toml_overlay_parses_runtime_table() {
        let raw = r#"
            # comment
            [runtime]
            host = "127.0.0.1"
            port = 12345
            data_dir = "/tmp/stuchka"
            first_run = false
        "#;
        let cfg = RuntimeConfig::from_toml_str(raw).unwrap();
        assert_eq!(cfg.port, 12345);
        assert_eq!(cfg.data_dir, PathBuf::from("/tmp/stuchka"));
        assert!(!cfg.first_run);
    }

    #[test]
    fn normalize_forces_loopback() {
        let mut cfg = RuntimeConfig {
            host: "0.0.0.0".to_string(),
            ..RuntimeConfig::default()
        };
        cfg.normalize();
        assert_eq!(cfg.host, "127.0.0.1", "INV-05: never bind 0.0.0.0");
    }

    #[test]
    fn bad_port_value_errors() {
        let raw = "[runtime]\nport = not_a_number\n";
        assert!(RuntimeConfig::from_toml_str(raw).is_err());
    }

    #[test]
    fn inject_no_proxy_covers_loopback() {
        std::env::remove_var("NO_PROXY");
        inject_no_proxy();
        let v = std::env::var("NO_PROXY").unwrap();
        assert!(v.contains("127.0.0.1"), "got {v}");
        assert!(v.contains("localhost"), "got {v}");
    }

    #[test]
    fn bind_addr_formats_host_port() {
        let cfg = RuntimeConfig {
            host: "127.0.0.1".to_string(),
            port: 0,
            ..RuntimeConfig::default()
        };
        assert_eq!(cfg.bind_addr(), "127.0.0.1:0");
    }
}
