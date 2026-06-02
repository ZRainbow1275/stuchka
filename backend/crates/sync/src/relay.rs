//! Sync-2 cross-network relay (03-sync-yjs §3.9) — **R2 seam, declared only**.
//!
//! R1 ships nothing here: the public relay path is blocked by L0-02 (备案豁免 legal opinion). The
//! trait is defined so the api router and the rest of the engine can name the type now; every
//! implementation is `unimplemented!` until R2.

use async_trait::async_trait;

use crate::error::SyncResult;

/// A cross-network relay provider (§3.9). Three R2 implementations are envisaged:
/// `TailscaleAdapter` / `CfTunnelAdapter` (user-operated) and `PublicYjsRelay` (project-operated,
/// requires the L0-02 备案 legal opinion before it may be built).
#[async_trait]
pub trait RelayProvider: Send + Sync {
    /// Connect the relay for a document. R1: every implementation panics with the L0-02 block.
    async fn connect(&self, doc_id: &str) -> SyncResult<()>;
}

/// R2 placeholder for a user-operated Tailscale relay.
pub struct TailscaleAdapter;

#[async_trait]
impl RelayProvider for TailscaleAdapter {
    async fn connect(&self, _doc_id: &str) -> SyncResult<()> {
        unimplemented!("R2: Sync-2 relay is blocked by L0-02 (备案豁免 legal opinion)")
    }
}

/// R2 placeholder for a user-operated Cloudflare-tunnel relay.
pub struct CfTunnelAdapter;

#[async_trait]
impl RelayProvider for CfTunnelAdapter {
    async fn connect(&self, _doc_id: &str) -> SyncResult<()> {
        unimplemented!("R2: Sync-2 relay is blocked by L0-02 (备案豁免 legal opinion)")
    }
}

/// R2 placeholder for the project-operated public Yjs relay (requires L0-02 备案).
pub struct PublicYjsRelay;

#[async_trait]
impl RelayProvider for PublicYjsRelay {
    async fn connect(&self, _doc_id: &str) -> SyncResult<()> {
        unimplemented!("R2: PublicYjsRelay requires the L0-02 备案豁免 legal opinion before build")
    }
}
