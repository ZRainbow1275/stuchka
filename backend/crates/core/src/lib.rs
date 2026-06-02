//! `core` — boot / spawn child process / READY handshake / first-run (D1).
//!
//! Owns the D1 process contract that `crates/api` consumes:
//! - [`config`]: runtime config from `config/secrets.toml` + env, with `NO_PROXY` injection
//!   and the INV-05 loopback guarantee;
//! - [`spawn`]: `127.0.0.1:0` bind + 64-byte hex token + the exact `READY{...}` stdout line;
//! - [`boot`]: the end-to-end boot sequence ([`boot::Boot::prepare`]);
//! - [`first_run`]: the first-run wizard step schema placeholder.

pub mod boot;
pub mod config;
pub mod first_run;
pub mod spawn;

pub use boot::{Boot, BootServices};
pub use config::{inject_no_proxy, RuntimeConfig, LOOPBACK_HOST, NO_PROXY_VALUE};
pub use first_run::{FirstRunStep, FirstRunWizard};
pub use spawn::{bind_loopback, generate_token, Handshake, READY_PREFIX, TOKEN_BYTES};

/// Crate identity for boot diagnostics and CI dependency-graph assertions.
pub const CRATE_NAME: &str = "core";

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_is_stable() {
        assert_eq!(super::CRATE_NAME, "core");
    }
}
