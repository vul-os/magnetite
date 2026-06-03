// In-house, offline IP geolocation.
//
// When `GEOIP_DB_PATH` points at a MaxMind GeoLite2-City `.mmdb` file the
// resolver reads it locally on every lookup — no network calls, so user IPs
// never leave the host. When the file is absent or unset, lookups degrade
// gracefully: private/loopback addresses are labelled "Local" and everything
// else returns an empty `GeoLocation` (NULL geo columns). This keeps the
// analytics pipeline honest rather than guessing.

use std::net::IpAddr;
use std::str::FromStr;

use maxminddb::{geoip2, Reader};

#[derive(Debug, Clone, Default)]
pub struct GeoLocation {
    pub country: Option<String>, // ISO-3166 alpha-2
    pub region: Option<String>,
    pub city: Option<String>,
}

impl GeoLocation {
    #[allow(dead_code)] // used by tests + a convenience for callers
    pub fn is_empty(&self) -> bool {
        self.country.is_none() && self.region.is_none() && self.city.is_none()
    }
}

pub struct GeoResolver {
    reader: Option<Reader<Vec<u8>>>,
}

impl GeoResolver {
    /// Build a resolver from `GEOIP_DB_PATH`. Always succeeds: a missing or
    /// unreadable database simply disables enrichment (logged once).
    pub fn from_env() -> Self {
        let reader = match std::env::var("GEOIP_DB_PATH") {
            Ok(path) if !path.is_empty() => match Reader::open_readfile(&path) {
                Ok(r) => {
                    tracing::info!("GeoIP database loaded from {path}");
                    Some(r)
                }
                Err(e) => {
                    tracing::warn!(
                        "GEOIP_DB_PATH set but database failed to load ({e}); \
                                    geo enrichment disabled"
                    );
                    None
                }
            },
            _ => None,
        };
        Self { reader }
    }

    /// Whether a geo database is loaded (drives the "geo enabled" hint in the UI).
    pub fn enabled(&self) -> bool {
        self.reader.is_some()
    }

    /// Resolve a textual IP to a coarse location. Never errors.
    pub fn lookup(&self, ip: &str) -> GeoLocation {
        let addr = match IpAddr::from_str(ip.trim()) {
            Ok(a) => a,
            Err(_) => return GeoLocation::default(),
        };

        if is_private(&addr) {
            return GeoLocation {
                country: Some("Local".to_string()),
                region: None,
                city: None,
            };
        }

        let Some(reader) = &self.reader else {
            return GeoLocation::default();
        };

        match reader.lookup::<geoip2::City>(addr) {
            Ok(rec) => GeoLocation {
                country: rec.country.and_then(|c| c.iso_code).map(|s| s.to_string()),
                region: rec
                    .subdivisions
                    .and_then(|subs| subs.into_iter().next())
                    .and_then(|s| s.names)
                    .and_then(|names| names.get("en").map(|s| s.to_string())),
                city: rec
                    .city
                    .and_then(|c| c.names)
                    .and_then(|names| names.get("en").map(|s| s.to_string())),
            },
            // AddressNotFound and decode errors both mean "unknown" — not fatal.
            Err(_) => GeoLocation::default(),
        }
    }
}

fn is_private(addr: &IpAddr) -> bool {
    match addr {
        IpAddr::V4(v4) => {
            v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_unspecified()
        }
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_database_returns_empty_for_public_ip() {
        let r = GeoResolver { reader: None };
        assert!(r.lookup("8.8.8.8").is_empty());
        assert!(!r.enabled());
    }

    #[test]
    fn private_addresses_are_labelled_local() {
        let r = GeoResolver { reader: None };
        assert_eq!(r.lookup("127.0.0.1").country.as_deref(), Some("Local"));
        assert_eq!(r.lookup("10.1.2.3").country.as_deref(), Some("Local"));
        assert_eq!(r.lookup("192.168.0.1").country.as_deref(), Some("Local"));
    }

    #[test]
    fn garbage_ip_is_empty() {
        let r = GeoResolver { reader: None };
        assert!(r.lookup("not-an-ip").is_empty());
    }
}
