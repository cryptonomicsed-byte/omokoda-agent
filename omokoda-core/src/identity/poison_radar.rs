//! Poison Radar — static on-chain risk heuristics for agent wallet addresses.
//! Ported from vanity-cloakseed's `poisonRadar.ts`.
//!
//! Two tiers of analysis:
//! - **Static** (`analyze_static`): zero-network heuristics — runs at birth,
//!   offline. Catches burn addresses, known-bad contracts, repeated-char
//!   runs, sequential byte patterns.
//! - **`PoisonReport`** serialises into the genesis manifest so every
//!   address risk fingerprint is Merkle-auditable from day one via GIX.
//!
//! Dynamic RPC analysis (fetching live tx history) is a future tool
//! capability; it requires async network access and does not belong in the
//! birth pipeline.

use serde::{Deserialize, Serialize};

/// Risk level for an address scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    #[default]
    None,
    Low,
    Medium,
    High,
    Unknown,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Suspicious pattern counts detected during static analysis.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SuspiciousPatterns {
    /// Longest run of the same character in the hex address.
    pub repeated_chars: u32,
    /// Whether a 6+ digit ascending or descending sequence was found.
    pub sequential_bytes: u32,
    /// Non-zero if the address matches a known-bad or burn address.
    pub known_bad: u32,
    /// Non-zero if the address has an anomalous all-zero prefix (beyond leading zeros).
    pub zero_prefix_anomaly: u32,
}

/// Full result of Poison Radar analysis for a single address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoisonReport {
    /// `"clean"` | `"suspicious"` | `"error"`
    pub status: String,
    pub risk: RiskLevel,
    pub risk_score: u32,
    pub patterns: SuspiciousPatterns,
    pub warnings: Vec<String>,
    pub message: String,
}

impl Default for PoisonReport {
    fn default() -> Self {
        Self::clean()
    }
}

impl PoisonReport {
    pub fn clean() -> Self {
        Self {
            status: "clean".into(),
            risk: RiskLevel::None,
            risk_score: 0,
            patterns: SuspiciousPatterns::default(),
            warnings: vec![],
            message: "Address appears clean".into(),
        }
    }

    pub fn error(msg: &str) -> Self {
        Self {
            status: "error".into(),
            risk: RiskLevel::Unknown,
            risk_score: 0,
            patterns: SuspiciousPatterns::default(),
            warnings: vec![],
            message: msg.to_string(),
        }
    }
}

/// Known high-risk Ethereum addresses (burn sinks and zero-address).
/// Legitimate tokens (USDT, USDC, etc.) are intentionally excluded — their
/// full address is used as a payment *destination*, not a source of risk.
static KNOWN_BAD_ETH: &[&str] = &[
    "0x0000000000000000000000000000000000000000",
    "0x000000000000000000000000000000000000dead",
];

/// Analyze an address with zero-network static heuristics.
///
/// `chain` is one of `"eth"`, `"ethereum"`, `"sol"`, `"solana"`, `"btc"`,
/// `"bitcoin"`, `"sui"`, `"cosmos"`, `"aptos"`, `"nostr"`, `"minipae"`.
pub fn analyze_static(address: &str, chain: &str) -> PoisonReport {
    if address.is_empty() {
        return PoisonReport::error("empty address");
    }

    let mut risk_score: u32 = 0;
    let mut warnings: Vec<String> = Vec::new();
    let mut patterns = SuspiciousPatterns::default();

    let addr_lower = address.to_lowercase();

    // 1. Known-bad address check (Ethereum only — chain-specific).
    if matches!(chain, "eth" | "ethereum") && KNOWN_BAD_ETH.iter().any(|bad| addr_lower == **bad) {
        patterns.known_bad += 1;
        risk_score += 30;
        warnings.push("Known burn or zero address".into());
    }

    // 2. Repeated-character analysis — longest hex-digit run.
    let hex_chars: Vec<char> = address
        .trim_start_matches("0x")
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect();

    if !hex_chars.is_empty() {
        let mut max_run = 1u32;
        let mut cur_run = 1u32;
        for i in 1..hex_chars.len() {
            if hex_chars[i] == hex_chars[i - 1] {
                cur_run += 1;
                if cur_run > max_run {
                    max_run = cur_run;
                }
            } else {
                cur_run = 1;
            }
        }
        if max_run >= 8 {
            patterns.repeated_chars = max_run;
            risk_score += (max_run - 7) * 2;
            warnings.push(format!("Repeated character run of {max_run} detected"));
        }
    }

    // 3. Anomalous all-zero prefix (8+ hex digits of zeros past any leading "0x").
    let stripped = address.trim_start_matches("0x");
    if stripped.len() >= 8 && stripped.chars().take(8).all(|c| c == '0') {
        patterns.zero_prefix_anomaly = 1;
        risk_score += 5;
        warnings.push("Anomalous all-zero prefix".into());
    }

    // 4. Sequential byte pattern (6+ ascending or descending hex nibbles in a row).
    if hex_chars.len() >= 6 {
        let asc = hex_chars.windows(6).any(|w| {
            w.windows(2).all(|p| {
                let a = p[0].to_digit(16).unwrap_or(0);
                let b = p[1].to_digit(16).unwrap_or(0);
                b == a.wrapping_add(1)
            })
        });
        let desc = hex_chars.windows(6).any(|w| {
            w.windows(2).all(|p| {
                let a = p[0].to_digit(16).unwrap_or(0);
                let b = p[1].to_digit(16).unwrap_or(0);
                a == b.wrapping_add(1)
            })
        });
        if asc || desc {
            patterns.sequential_bytes = 1;
            risk_score += 3;
            warnings.push("Sequential nibble pattern (possible dust address)".into());
        }
    }

    let risk = if risk_score >= 20 {
        RiskLevel::High
    } else if risk_score >= 10 {
        RiskLevel::Medium
    } else if risk_score > 0 {
        RiskLevel::Low
    } else {
        RiskLevel::None
    };

    let status = if risk == RiskLevel::High {
        "suspicious".to_string()
    } else {
        "clean".to_string()
    };
    let message = if warnings.is_empty() {
        "Address appears clean".to_string()
    } else {
        warnings.join(", ")
    };

    PoisonReport {
        status,
        risk,
        risk_score,
        patterns,
        warnings,
        message,
    }
}

/// Scan a slice of (chain, address) pairs and return one `PoisonReport` each.
pub fn scan_wallet_addresses(wallets: &[(&str, &str)]) -> Vec<(String, PoisonReport)> {
    wallets
        .iter()
        .map(|(chain, addr)| (chain.to_string(), analyze_static(addr, chain)))
        .collect()
}

/// One-line summary: `"eth: clean (0)"` or `"eth: suspicious (25) — Burn address"`.
pub fn format_report(chain: &str, report: &PoisonReport) -> String {
    if report.warnings.is_empty() {
        format!("{chain}: {} ({})", report.status, report.risk_score)
    } else {
        format!(
            "{chain}: {} ({}) — {}",
            report.status,
            report.risk_score,
            report.warnings.join("; ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_eth_address() {
        let r = analyze_static("0x71C7656EC7ab88b098defB751B7401B5f6d8976F", "eth");
        assert_eq!(r.risk, RiskLevel::None);
        assert_eq!(r.status, "clean");
    }

    #[test]
    fn burn_address_flagged() {
        let r = analyze_static("0x0000000000000000000000000000000000000000", "eth");
        assert!(
            r.risk_score >= 20,
            "burn address should be high risk, got {}",
            r.risk_score
        );
        assert_eq!(r.risk, RiskLevel::High);
        assert_eq!(r.status, "suspicious");
    }

    #[test]
    fn repeated_char_run_flagged() {
        // 8 consecutive 'a' chars → max_run = 8 → risk_score += 2
        let r = analyze_static("0xaaaaaaaabb71C7656EC7ab88b098defB751B7401", "eth");
        assert!(
            r.patterns.repeated_chars >= 8,
            "got {}",
            r.patterns.repeated_chars
        );
        assert!(r.risk_score > 0);
    }

    #[test]
    fn zero_prefix_flagged() {
        let r = analyze_static("0x00000000deadbeef71C7656EC7ab88b098defB75", "eth");
        assert_eq!(r.patterns.zero_prefix_anomaly, 1);
    }

    #[test]
    fn solana_address_not_known_bad() {
        // Solana addresses use base58, no "0x" prefix — format check still works
        let r = analyze_static("3Jv8SQcRQsKqnbkRaByRGJRumQFcMGcApKAC6E7X1234", "sol");
        assert_eq!(r.patterns.known_bad, 0);
    }

    #[test]
    fn scan_multiple_wallets() {
        let wallets = vec![
            ("eth", "0x71C7656EC7ab88b098defB751B7401B5f6d8976F"),
            ("sol", "3Jv8SQcRQsKqnbkRaByRGJRumQFcMGcApKAC6E7X1234"),
            ("btc", "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"),
        ];
        let results = scan_wallet_addresses(&wallets);
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|(_, r)| r.status != "error"));
    }

    #[test]
    fn format_report_clean() {
        let s = format_report("eth", &PoisonReport::clean());
        assert!(s.contains("eth: clean (0)"), "got: {s}");
    }
}
