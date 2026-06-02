//! SY-11: mDNS advertise/browse on `_stuchka._tcp.local.` with link-local filtering (§3.6 / §3.10).
//!
//! Network-dependent: this binds real multicast sockets, so the round-trip test is `#[ignore]`d and
//! run explicitly (`cargo test -p sync --test mdns_discovery -- --ignored`). The link-local filter
//! itself is asserted with no sockets so the privacy invariant is always in the default gate.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use sync::{advertise, browse, is_link_local, SERVICE_TYPE};

#[test]
fn sy11_link_local_filter_excludes_public_ips() {
    // Default-gate invariant (no sockets): only link-local / private / loopback are advertisable.
    assert!(is_link_local(&IpAddr::V4(Ipv4Addr::new(169, 254, 9, 9))));
    assert!(is_link_local(&IpAddr::V4(Ipv4Addr::new(192, 168, 0, 2))));
    assert!(is_link_local(&IpAddr::V6(Ipv6Addr::LOCALHOST)));
    assert!(!is_link_local(&IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1))));
    assert!(!is_link_local(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
}

#[test]
fn sy11_service_type_is_stuchka() {
    assert_eq!(SERVICE_TYPE, "_stuchka._tcp.local.");
}

#[tokio::test]
#[ignore = "binds real mDNS multicast sockets; run explicitly with --ignored"]
async fn sy11_advertise_then_browse_resolves_self() {
    // Advertise this node, browse for the service type, and assert we resolve our own instance.
    let peer_id = "test-peer-abc";
    let _daemon_adv = advertise(45999, peer_id, "stuchka-test").expect("advertise");

    let browser = mdns_sd::ServiceDaemon::new().expect("browse daemon");
    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    browse(&browser, tx).expect("browse");

    let resolved = tokio::time::timeout(Duration::from_secs(8), async {
        while let Some(peer) = rx.recv().await {
            // Any discovered peer must carry a link-local address (or none).
            if let Some(ip) = peer.ip {
                assert!(is_link_local(&ip), "discovered a non-link-local ip: {ip}");
            }
            if peer.peer_id.contains(peer_id) {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);

    assert!(resolved, "should resolve our own advertised service");
}
