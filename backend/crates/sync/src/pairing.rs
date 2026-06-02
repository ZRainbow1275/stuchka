//! Device pairing (trust on first use, 03-sync-yjs §3.6.2). R1a freezes the request shape and the
//! TOFU flow (PIN verify -> mark trusted + store the peer's ed25519 public key); the desktop PIN
//! display + 30s expiry timer UX is an R1b refinement layered on this same seam.
//!
//! The signature binds the PIN to the peer key so a replayed `PairRequest` for a different key
//! fails: the peer signs `peer_id|pin` with its own key.

use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::error::{SyncError, SyncResult};
use crate::repo::PeerRepo;

/// A pairing request from a connecting device (§3.6.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairRequest {
    /// The peer's base58 ed25519 public key.
    pub peer_id: String,
    /// The 6-digit PIN shown on the desktop.
    pub pin: String,
    /// ed25519 signature (base64) over `peer_id|pin`.
    pub signature: String,
}

impl PairRequest {
    fn signing_payload(&self) -> Vec<u8> {
        format!("{}|{}", self.peer_id, self.pin).into_bytes()
    }
}

fn decode_pubkey(peer_id: &str) -> Option<VerifyingKey> {
    let bytes = bs58::decode(peer_id).into_vec().ok()?;
    let arr: [u8; 32] = bytes.try_into().ok()?;
    VerifyingKey::from_bytes(&arr).ok()
}

fn decode_signature(b64: &str) -> Option<Signature> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    Signature::from_slice(&bytes).ok()
}

/// Handle a pairing request (§3.6.2): check the PIN against `expected_pin`, verify the peer's
/// signature over `peer_id|pin`, then mark the peer trusted and store its public key.
///
/// PIN expiry (30s) is enforced by the caller that issued `expected_pin`; here we only compare.
pub async fn handle_pair_request(
    peers: &dyn PeerRepo,
    req: &PairRequest,
    expected_pin: &str,
) -> SyncResult<()> {
    if req.pin != expected_pin {
        return Err(SyncError::Handshake("pairing PIN mismatch".into()));
    }
    let key = decode_pubkey(&req.peer_id)
        .ok_or_else(|| SyncError::Handshake("malformed peer key".into()))?;
    let sig = decode_signature(&req.signature)
        .ok_or_else(|| SyncError::Handshake("malformed pairing signature".into()))?;
    key.verify_strict(&req.signing_payload(), &sig)
        .map_err(|_| SyncError::Handshake("pairing signature invalid".into()))?;
    // TOFU: trust the peer and remember its key (which is the peer id itself).
    peers
        .set_trusted(&req.peer_id, true, Some(&req.peer_id))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::{ensure_schema, SqlitePeerRepo};
    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    async fn peer_repo() -> SqlitePeerRepo {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        ensure_schema(&pool).await.unwrap();
        SqlitePeerRepo::new(pool)
    }

    fn request(seed: u8, pin: &str) -> PairRequest {
        let sk = SigningKey::from_bytes(&[seed; 32]);
        let peer_id = bs58::encode(sk.verifying_key().to_bytes()).into_string();
        let mut req = PairRequest {
            peer_id,
            pin: pin.to_string(),
            signature: String::new(),
        };
        let sig = sk.sign(&req.signing_payload());
        req.signature = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());
        req
    }

    #[tokio::test]
    async fn correct_pin_and_signature_trusts_peer() {
        let peers = peer_repo().await;
        let req = request(3, "123456");
        handle_pair_request(&peers, &req, "123456").await.unwrap();
        assert!(peers.is_trusted(&req.peer_id).await.unwrap());
    }

    #[tokio::test]
    async fn wrong_pin_is_rejected() {
        let peers = peer_repo().await;
        let req = request(3, "000000");
        let err = handle_pair_request(&peers, &req, "123456")
            .await
            .unwrap_err();
        assert!(matches!(err, SyncError::Handshake(_)));
        assert!(!peers.is_trusted(&req.peer_id).await.unwrap());
    }
}
