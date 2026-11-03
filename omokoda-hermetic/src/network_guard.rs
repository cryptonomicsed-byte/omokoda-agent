use serde::{Deserialize, Serialize};

/// Decision from the network guard
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkGuardDecision {
    Allow,
    Warn(String),
    Block(String),
}

impl NetworkGuardDecision {
    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Block(_))
    }
}

/// Configuration for the network guard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkGuardConfig {
    /// Block RFC-1918 / link-local / loopback targets
    pub block_private_ranges: bool,
    /// Block cloud metadata endpoints (169.254.169.254, etc.)
    pub block_metadata_endpoints: bool,
    /// Explicit allowlist of domains (empty = allow all non-blocked)
    pub domain_allowlist: Vec<String>,
    /// Explicit blocklist of domains
    pub domain_blocklist: Vec<String>,
}

impl Default for NetworkGuardConfig {
    fn default() -> Self {
        Self {
            block_private_ranges: true,
            block_metadata_endpoints: true,
            domain_allowlist: Vec::new(),
            domain_blocklist: Vec::new(),
        }
    }
}

/// SSRF Guard — validates network-bound requests against known attack patterns.
/// Ports `ssrfGuard.ts`.
pub struct NetworkGuard {
    config: NetworkGuardConfig,
}

impl Default for NetworkGuard {
    fn default() -> Self {
        Self::new(NetworkGuardConfig::default())
    }
}

impl NetworkGuard {
    pub fn new(config: NetworkGuardConfig) -> Self {
        Self { config }
    }

    /// Check a URL or hostname for SSRF risk.
    /// Returns Allow, Warn, or Block with a reason.
    pub fn check(&self, url: &str) -> NetworkGuardDecision {
        let normalized = url.trim().to_lowercase();

        // 1. Domain blocklist (exact + suffix match)
        if let Some(reason) = self.check_domain_blocklist(&normalized) {
            return NetworkGuardDecision::Block(reason);
        }

        // 2. Cloud metadata endpoints (highest priority block)
        if self.config.block_metadata_endpoints {
            if let Some(reason) = self.check_metadata_endpoints(&normalized) {
                return NetworkGuardDecision::Block(reason);
            }
        }

        // 3. Private/loopback IP ranges
        if self.config.block_private_ranges {
            if let Some(reason) = self.check_private_ranges(&normalized) {
                return NetworkGuardDecision::Block(reason);
            }
        }

        // 4. Suspicious redirect indicators
        if let Some(reason) = self.check_redirect_indicators(&normalized) {
            return NetworkGuardDecision::Warn(reason);
        }

        // 5. Domain allowlist enforcement
        if !self.config.domain_allowlist.is_empty() {
            let host = extract_host(&normalized);
            let allowed = self
                .config
                .domain_allowlist
                .iter()
                .any(|allowed| host_matches(host, allowed));
            if !allowed {
                return NetworkGuardDecision::Block(format!(
                    "SSRF guard: '{}' not in domain allowlist",
                    host
                ));
            }
        }

        NetworkGuardDecision::Allow
    }

    fn check_metadata_endpoints(&self, url: &str) -> Option<String> {
        let metadata_patterns = [
            "169.254.169.254", // AWS/GCP/Azure IMDS
            "metadata.google.internal",
            "169.254.170.2",   // ECS task metadata
            "100.100.100.200", // Alibaba Cloud IMDS
            "192.0.0.192",     // Oracle Cloud IMDS
        ];
        for pattern in &metadata_patterns {
            if url.contains(pattern) {
                return Some(format!(
                    "SSRF guard: blocked cloud metadata endpoint '{}'",
                    pattern
                ));
            }
        }
        None
    }

    fn check_private_ranges(&self, url: &str) -> Option<String> {
        let host = extract_host(url);

        // Hostname forms.
        if host == "localhost" || host.ends_with(".localhost") || host == "::1" || host == "[::1]" {
            return Some(format!("SSRF guard: loopback address blocked '{}'", host));
        }

        // IP forms, including the non-canonical spellings a string check misses
        // (decimal 2130706433, 0x7f.1, [::ffff:127.0.0.1], octal octets).
        if let Some(ip) = parse_host_ip(host) {
            let blocked = match ip {
                std::net::IpAddr::V4(v4) => {
                    v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
                }
                std::net::IpAddr::V6(v6) => {
                    let seg = v6.segments();
                    let ula = (seg[0] & 0xfe00) == 0xfc00;
                    let link_local = (seg[0] & 0xffc0) == 0xfe80;
                    v6.is_loopback() || v6.is_unspecified() || ula || link_local
                }
            };
            if blocked {
                return Some(format!(
                    "SSRF guard: private/loopback address blocked '{}' ({})",
                    host, ip
                ));
            }
        }

        None
    }

    fn check_domain_blocklist(&self, url: &str) -> Option<String> {
        let host = extract_host(url);
        for blocked in &self.config.domain_blocklist {
            if host_matches(host, blocked) {
                return Some(format!("SSRF guard: domain '{}' is in blocklist", host));
            }
        }
        None
    }

    fn check_redirect_indicators(&self, url: &str) -> Option<String> {
        // Detect URL redirectors that might bypass host checks
        let suspicious = [
            "redirect=",
            "next=",
            "url=",
            "target=",
            "dest=",
            "forward=",
            "return=",
        ];
        for indicator in &suspicious {
            if url.contains(indicator) {
                return Some(format!(
                    "SSRF guard: redirect parameter '{}' may enable SSRF",
                    indicator
                ));
            }
        }

        // Detect double-encoded slashes or protocol confusion
        if url.contains("@") && url.contains("://") {
            return Some("SSRF guard: URL with credentials may obscure actual target".to_string());
        }

        None
    }
}

/// Extract the host portion from a URL string (best-effort, no full URL parsing).
fn extract_host(url: &str) -> &str {
    // Strip scheme
    let after_scheme = if let Some(pos) = url.find("://") {
        &url[pos + 3..]
    } else {
        url
    };

    // Strip path, query, fragment
    let host_port = after_scheme
        .split('/')
        .next()
        .unwrap_or(after_scheme)
        .split('?')
        .next()
        .unwrap_or(after_scheme)
        .split('#')
        .next()
        .unwrap_or(after_scheme);

    // Strip credentials. `http://evil.com@127.0.0.1/` is a request to 127.0.0.1,
    // but the previous code returned the whole `evil.com@127.0.0.1` authority,
    // so the loopback host was allowed through unchecked. Everything before the
    // LAST '@' is userinfo.
    let host_port = match host_port.rfind('@') {
        Some(pos) => &host_port[pos + 1..],
        None => host_port,
    };

    // Strip port
    // Handle IPv6 [::1]:port
    if host_port.starts_with('[') {
        if let Some(end) = host_port.find(']') {
            return &host_port[..=end];
        }
    }
    host_port.split(':').next().unwrap_or(host_port)
}

/// Parse the host into an IP address, resolving the non-canonical spellings a
/// URL parser accepts but a string prefix check misses: decimal (`2130706433`),
/// hex (`0x7f000001`), octal-ish (`0177.0.0.1`) and IPv4-mapped IPv6
/// (`[::ffff:127.0.0.1]`).
fn parse_host_ip(host: &str) -> Option<std::net::IpAddr> {
    let h = host.trim_start_matches('[').trim_end_matches(']');

    if let Ok(ip) = h.parse::<std::net::IpAddr>() {
        // Normalise ::ffff:127.0.0.1 to its IPv4 form.
        if let std::net::IpAddr::V6(v6) = ip {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return Some(std::net::IpAddr::V4(v4));
            }
        }
        return Some(ip);
    }

    // inet_aton-style shorthand: a 32-bit value, a.b (8+24), a.b.c (8+8+16)
    // and a.b.c.d all name an IPv4 address. "127.1" means 127.0.0.1.
    let parts: Vec<&str> = h.split('.').collect();
    if parts.is_empty() || parts.len() > 4 {
        return None;
    }
    let parse_part = |p: &str| -> Option<u64> {
        if p.is_empty() {
            return None;
        }
        if let Some(hex) = p.strip_prefix("0x").or_else(|| p.strip_prefix("0X")) {
            u64::from_str_radix(hex, 16).ok()
        } else if p.len() > 1 && p.starts_with('0') {
            u64::from_str_radix(&p[1..], 8).ok()
        } else {
            p.parse::<u64>().ok()
        }
    };
    let mut vals = [0u64; 4];
    for (i, part) in parts.iter().enumerate() {
        vals[i] = parse_part(part)?;
    }
    let value: u64 = match parts.len() {
        1 => vals[0],
        2 if vals[0] <= 255 && vals[1] <= 0x00ff_ffff => (vals[0] << 24) | vals[1],
        3 if vals[0] <= 255 && vals[1] <= 255 && vals[2] <= 0xffff => {
            (vals[0] << 24) | (vals[1] << 16) | vals[2]
        }
        4 if vals.iter().all(|v| *v <= 255) => {
            (vals[0] << 24) | (vals[1] << 16) | (vals[2] << 8) | vals[3]
        }
        _ => return None,
    };
    if value > u32::MAX as u64 {
        return None;
    }
    Some(std::net::IpAddr::V4(std::net::Ipv4Addr::from(value as u32)))
}

/// Check if a host matches an allowlist/blocklist entry (exact or suffix `.example.com`)
fn host_matches(host: &str, pattern: &str) -> bool {
    if host == pattern {
        return true;
    }
    // Suffix match: pattern ".example.com" matches "api.example.com"
    if let Some(suffix) = pattern.strip_prefix('.') {
        return host == suffix || host.ends_with(&format!(".{}", suffix));
    }
    // Pattern without leading dot: exact or subdomain
    host == pattern || host.ends_with(&format!(".{}", pattern))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> NetworkGuard {
        NetworkGuard::default()
    }

    #[test]
    fn test_block_localhost() {
        assert!(guard().check("http://localhost/api").is_blocked());
        assert!(guard().check("http://127.0.0.1/admin").is_blocked());
    }

    #[test]
    fn test_block_metadata() {
        assert!(guard()
            .check("http://169.254.169.254/latest/meta-data/")
            .is_blocked());
        assert!(guard()
            .check("http://metadata.google.internal/computeMetadata/v1/")
            .is_blocked());
    }

    #[test]
    fn test_block_private_ranges() {
        assert!(guard().check("http://10.0.0.1/secret").is_blocked());
        assert!(guard().check("http://192.168.1.1/").is_blocked());
        assert!(guard().check("http://172.16.0.1/").is_blocked());
        assert!(guard().check("http://172.31.255.255/").is_blocked());
        assert!(!guard().check("http://172.32.0.1/").is_blocked());
    }

    #[test]
    fn test_allow_public_url() {
        assert_eq!(
            guard().check("https://api.example.com/v1"),
            NetworkGuardDecision::Allow
        );
    }

    #[test]
    fn test_allowlist_enforcement() {
        let guard = NetworkGuard::new(NetworkGuardConfig {
            domain_allowlist: vec!["api.example.com".to_string()],
            ..Default::default()
        });
        assert_eq!(
            guard.check("https://api.example.com/v1"),
            NetworkGuardDecision::Allow
        );
        assert!(guard.check("https://evil.com/").is_blocked());
    }

    #[test]
    fn test_redirect_param_warns() {
        let decision = guard().check("https://safe.com/?redirect=http://evil.com");
        assert!(matches!(decision, NetworkGuardDecision::Warn(_)));
    }

    #[test]
    fn test_blocklist() {
        let guard = NetworkGuard::new(NetworkGuardConfig {
            domain_blocklist: vec!["evil.com".to_string()],
            ..Default::default()
        });
        assert!(guard.check("https://evil.com/payload").is_blocked());
        assert!(guard.check("https://sub.evil.com/payload").is_blocked());
    }

    /// Regression: userinfo made the authority string start with a public host,
    /// so `http://evil.com@127.0.0.1/` reached loopback unchecked.
    #[test]
    fn test_userinfo_cannot_hide_loopback() {
        assert!(guard().check("http://evil.com@127.0.0.1/admin").is_blocked());
        assert!(guard()
            .check("http://user:pass@127.0.0.1:8000/seed")
            .is_blocked());
        assert!(guard()
            .check("http://169.254.169.254@example.com/")
            .is_blocked());
    }

    /// Regression: alternate IP spellings bypassed the string-prefix checks.
    #[test]
    fn test_non_canonical_ip_spellings_are_blocked() {
        for url in [
            "http://2130706433/",            // 127.0.0.1 as decimal
            "http://0x7f000001/",            // 127.0.0.1 as hex
            "http://0177.0.0.1/",            // 127.0.0.1 as octal
            "http://[::ffff:127.0.0.1]/",    // IPv4-mapped IPv6
            "http://[::1]/",                 // IPv6 loopback
            "http://127.1/",                 // shorthand loopback
            "http://2852039166/latest/",     // 169.254.169.254 as decimal
        ] {
            assert!(guard().check(url).is_blocked(), "expected block for {url}");
        }
        // Public addresses keep working.
        assert_eq!(
            guard().check("https://93.184.216.34/x"),
            NetworkGuardDecision::Allow
        );
    }
}
