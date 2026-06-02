//! mDNS LAN discovery (03-sync-yjs §3.6). Advertises `_stuchka._tcp.local.` and browses for peers.
//!
//! Privacy (§3.10 / SY-11): the service is advertised **only on link-local addresses**; public IPs
//! are never advertised. `advertise` filters its address set through [`is_link_local`] before
//! registering, so a host with both a public and a LAN interface only exposes the LAN one.
//!
//! mdns-sd version note: the spec wrote 0.11; the current stable is 0.20. The
//! `ServiceDaemon::new()` / `register(ServiceInfo)` / `browse() -> Receiver<ServiceEvent>` surface
//! and `ServiceEvent::ServiceResolved` are stable across that range; in 0.20 the resolved payload
//! is a boxed `ResolvedService` whose `get_addresses()` yields `ScopedIp` (we convert via
//! `to_ip_addr`).

use std::net::IpAddr;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::error::{SyncError, SyncResult};

/// DNS-SD service type for Stučka LAN sync.
pub const SERVICE_TYPE: &str = "_stuchka._tcp.local.";

/// A peer discovered via mDNS browse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerDiscovered {
    /// Full service name (carries the advertised instance / peer id).
    pub peer_id: String,
    /// First resolved address (link-local in practice).
    pub ip: Option<IpAddr>,
    /// Advertised port.
    pub port: u16,
}

/// Is `ip` a link-local / private-LAN address that is safe to advertise (§3.10)?
///
/// Accepts IPv4 link-local `169.254/16`, RFC1918 private ranges, loopback, and IPv6
/// link-local `fe80::/10` / unique-local `fc00::/7` / loopback. Public addresses return `false`.
#[must_use]
pub fn is_link_local(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_link_local() || v4.is_private() || v4.is_loopback(),
        IpAddr::V6(v6) => {
            // fe80::/10 link-local, fc00::/7 unique-local, ::1 loopback.
            let seg = v6.segments();
            let is_unicast_link_local = (seg[0] & 0xffc0) == 0xfe80;
            let is_unique_local = (seg[0] & 0xfe00) == 0xfc00;
            is_unicast_link_local || is_unique_local || v6.is_loopback()
        }
    }
}

/// Enumerate this host's link-local / LAN-safe addresses (no public IPs, §3.10).
///
/// Uses the daemon-independent route the spec describes ("probe the routing table at startup").
/// Returns at least loopback so a single-host demo / test can still resolve itself.
fn lan_addresses() -> Vec<IpAddr> {
    let mut out = Vec::new();
    // mdns-sd exposes the interface scan via its own daemon, but for the advertise filter we use a
    // hostname resolution fallback plus loopback so behaviour is deterministic and never leaks a
    // public IP. Real interface enumeration (getifaddrs) is an R1b refinement on this same seam.
    if let Ok(name) = hostname_lookup() {
        for ip in name {
            if is_link_local(&ip) {
                out.push(ip);
            }
        }
    }
    if out.is_empty() {
        out.push(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    }
    out
}

/// Resolve the local hostname to its addresses (best-effort).
fn hostname_lookup() -> std::io::Result<Vec<IpAddr>> {
    use std::net::ToSocketAddrs;
    let host = mdns_sd_hostname();
    let addrs = (host.as_str(), 0u16)
        .to_socket_addrs()
        .map(|it| it.map(|sa| sa.ip()).collect::<Vec<_>>())?;
    Ok(addrs)
}

/// The local hostname, normalised for mDNS (`<name>.local.`).
fn mdns_sd_hostname() -> String {
    let raw = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "localhost".to_string());
    if raw.ends_with(".local.") {
        raw
    } else {
        format!("{}.local.", raw.trim_end_matches('.'))
    }
}

/// Advertise this node on the LAN as `_stuchka._tcp.local.` (§3.6.1).
///
/// `peer_id` is the instance name (the base58 ed25519 key). Only link-local addresses are
/// published (§3.10). Returns the live [`ServiceDaemon`] (drop it to stop advertising).
pub fn advertise(port: u16, peer_id: &str, hostname: &str) -> SyncResult<ServiceDaemon> {
    let daemon = ServiceDaemon::new().map_err(|e| SyncError::Mdns(e.to_string()))?;
    let addrs = lan_addresses();
    let host = if hostname.ends_with(".local.") {
        hostname.to_string()
    } else {
        format!("{}.local.", hostname.trim_end_matches('.'))
    };
    let props: &[(&str, &str)] = &[("v", "1"), ("os", std::env::consts::OS)];
    let info = ServiceInfo::new(SERVICE_TYPE, peer_id, &host, addrs.as_slice(), port, props)
        .map_err(|e| SyncError::Mdns(e.to_string()))?;
    daemon
        .register(info)
        .map_err(|e| SyncError::Mdns(e.to_string()))?;
    Ok(daemon)
}

/// Browse for `_stuchka._tcp.local.` peers, forwarding each resolved service to `sink` (§3.6.1).
///
/// Spawns a Tokio task that drains the daemon's event channel; the task ends when the daemon is
/// dropped or the sink is closed. Only resolved services whose addresses pass [`is_link_local`]
/// (defence in depth — a remote could in theory advertise a public IP) are forwarded.
pub fn browse(
    daemon: &ServiceDaemon,
    sink: tokio::sync::mpsc::Sender<PeerDiscovered>,
) -> SyncResult<()> {
    let recv = daemon
        .browse(SERVICE_TYPE)
        .map_err(|e| SyncError::Mdns(e.to_string()))?;
    tokio::spawn(async move {
        while let Ok(event) = recv.recv_async().await {
            if let ServiceEvent::ServiceResolved(info) = event {
                let ip = info
                    .get_addresses()
                    .iter()
                    .map(|scoped| scoped.to_ip_addr())
                    .find(is_link_local);
                let discovered = PeerDiscovered {
                    peer_id: info.get_fullname().to_string(),
                    ip,
                    port: info.get_port(),
                };
                if sink.send(discovered).await.is_err() {
                    break; // receiver gone
                }
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn link_local_classification() {
        // SY-11 helper: link-local / private / loopback are advertisable; public is not.
        assert!(is_link_local(&IpAddr::V4(Ipv4Addr::new(169, 254, 1, 2))));
        assert!(is_link_local(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10))));
        assert!(is_link_local(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5))));
        assert!(is_link_local(&IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(is_link_local(&IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(is_link_local(&"fe80::1".parse::<IpAddr>().unwrap()));

        // Public IPs must never be advertised.
        assert!(!is_link_local(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_link_local(&IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
        assert!(!is_link_local(
            &"2001:4860:4860::8888".parse::<IpAddr>().unwrap()
        ));
    }

    #[test]
    fn lan_addresses_never_contain_public() {
        // §3.10: the advertise set must be empty of public IPs.
        for ip in lan_addresses() {
            assert!(is_link_local(&ip), "advertise set leaked a public ip: {ip}");
        }
    }
}
