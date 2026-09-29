//! Walrus blob anchoring — Tier 3 of the fractal memory (`specs/memory-fractal.md`).
//!
//! Dense sub-graphs (REM fold archives, media) do not belong on-chain; they
//! belong in Walrus, with only the blob id + BLAKE3 hash anchored in the
//! receipt chain / on Sui. Anchor the hash, not the data.
//!
//! Configuration (fail-open — unset means "not configured", never a crash):
//!   WALRUS_PUBLISHER_URL   e.g. https://publisher.walrus-testnet.walrus.space
//!   WALRUS_AGGREGATOR_URL  e.g. https://aggregator.walrus-testnet.walrus.space
//!   WALRUS_EPOCHS          storage duration in epochs (default 5)
//!
//! Wire protocol (Walrus HTTP API):
//!   store  PUT {publisher}/v1/blobs?epochs=N   body = raw bytes
//!   read   GET {aggregator}/v1/blobs/{blob_id}
//!
//! The store response reports either `newlyCreated.blobObject.blobId` or
//! `alreadyCertified.blobId` (idempotent re-upload) — both are success.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::memory::memdir::OduDirectory;

pub const DEFAULT_EPOCHS: u32 = 5;

#[derive(Debug, Clone)]
pub struct WalrusConfig {
    pub publisher_url: String,
    pub aggregator_url: String,
    pub epochs: u32,
}

impl WalrusConfig {
    /// Read config from the environment. `None` = Walrus not configured on
    /// this runtime (the caller should skip anchoring, not fail).
    pub fn from_env() -> Option<Self> {
        let publisher_url = std::env::var("WALRUS_PUBLISHER_URL").ok()?;
        let aggregator_url = std::env::var("WALRUS_AGGREGATOR_URL").ok()?;
        if publisher_url.is_empty() || aggregator_url.is_empty() {
            return None;
        }
        let epochs = std::env::var("WALRUS_EPOCHS")
            .ok()
            .and_then(|e| e.parse().ok())
            .unwrap_or(DEFAULT_EPOCHS);
        Some(Self {
            publisher_url,
            aggregator_url,
            epochs,
        })
    }
}

/// What gets anchored in the receipt chain after a successful store: enough
/// to locate the blob (id) and to prove what it contained (hash), never the
/// data itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalrusAnchor {
    pub blob_id: String,
    /// BLAKE3 of the stored bytes — verifiable against a later `read_blob`.
    pub blake3_hex: String,
    pub byte_len: usize,
}

impl WalrusAnchor {
    /// Prefix used for splat blob labels in the receipt chain.
    pub const SPLAT_LABEL_PREFIX: &'static str = "splat:";

    /// Receipt payload for `record_receipt("walrus_anchor", …)` — the JSON
    /// that gets hash-committed into the agent's receipt chain.
    pub fn receipt_payload(&self, label: &str) -> String {
        serde_json::json!({
            "kind": "walrus_anchor",
            "label": label,
            "blob_id": self.blob_id,
            "blake3": self.blake3_hex,
            "byte_len": self.byte_len,
        })
        .to_string()
    }
}

/// Whether the blob stored in Walrus is a SOG-compressed splat or raw PLY.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SplatFormat {
    /// SOG-compressed Gaussian splat (smaller, produced by SplatTransform CLI).
    Sog,
    /// Raw PLY point-cloud (fallback when SplatTransform is not available).
    Ply,
}

/// Extended anchor for Gaussian splat blobs — wraps a [`WalrusAnchor`] and
/// carries splat-specific provenance fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplatAnchor {
    pub walrus: WalrusAnchor,
    /// Format of the bytes actually stored in Walrus.
    pub format: SplatFormat,
    /// `sog_bytes / ply_bytes`, or `1.0` when no compression was applied.
    pub compression_ratio: f32,
    /// BLAKE3 hex of the *original* PLY bytes (present even when SOG is stored).
    pub ply_blake3: String,
}

impl SplatAnchor {
    /// Receipt payload for `record_receipt("splat_anchor", …)`.
    /// Includes format and compression_ratio alongside the standard anchor fields.
    pub fn receipt_payload(&self, label: &str) -> String {
        let format_str = match self.format {
            SplatFormat::Sog => "sog",
            SplatFormat::Ply => "ply",
        };
        serde_json::json!({
            "kind": "splat_anchor",
            "label": label,
            "blob_id": self.walrus.blob_id,
            "blake3": self.walrus.blake3_hex,
            "byte_len": self.walrus.byte_len,
            "ply_blake3": self.ply_blake3,
            "format": format_str,
            "compression_ratio": self.compression_ratio,
        })
        .to_string()
    }
}

/// Extract the blob id from a Walrus publisher store response.
/// Pure — testable without a network.
pub fn parse_store_response(json: &serde_json::Value) -> Option<String> {
    if let Some(id) = json
        .pointer("/newlyCreated/blobObject/blobId")
        .and_then(|v| v.as_str())
    {
        return Some(id.to_string());
    }
    json.pointer("/alreadyCertified/blobId")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// Serialize one archived REM fold (macro node + its micro entries) into a
/// canonical JSON blob and its BLAKE3 hash. `None` if `macro_id` has no
/// archived fold. Pure — the export does not mutate the directory.
pub fn export_fold_json(dir: &OduDirectory, macro_id: &str) -> Option<(String, String)> {
    let micro = dir.archived_folds.get(macro_id)?;
    let macro_node = dir.entries.get(macro_id);
    let blob = serde_json::json!({
        "kind": "rem_fold_archive",
        "version": 1,
        "macro_id": macro_id,
        "macro_node": macro_node,
        "entries": micro,
    });
    let json = serde_json::to_string(&blob).ok()?;
    let hash = blake3::hash(json.as_bytes()).to_hex().to_string();
    Some((json, hash))
}

/// HTTP client for the Walrus publisher/aggregator pair.
pub struct WalrusClient {
    config: WalrusConfig,
    http: reqwest::Client,
}

impl WalrusClient {
    pub fn new(config: WalrusConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
        }
    }

    /// From env, or `None` when Walrus is not configured.
    pub fn from_env() -> Option<Self> {
        WalrusConfig::from_env().map(Self::new)
    }

    /// Store raw bytes; returns the anchor (blob id + hash + length).
    pub async fn store_blob(&self, bytes: Vec<u8>) -> Result<WalrusAnchor, String> {
        let blake3_hex = blake3::hash(&bytes).to_hex().to_string();
        let byte_len = bytes.len();
        let url = format!(
            "{}/v1/blobs?epochs={}",
            self.config.publisher_url.trim_end_matches('/'),
            self.config.epochs
        );
        let resp = self
            .http
            .put(&url)
            .body(bytes)
            .send()
            .await
            .map_err(|e| format!("walrus store failed: {e}"))?;
        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("walrus store: invalid response: {e}"))?;
        if !status.is_success() {
            return Err(format!("walrus store returned {status}: {body}"));
        }
        let blob_id = parse_store_response(&body)
            .ok_or_else(|| format!("walrus store: no blobId in response: {body}"))?;
        Ok(WalrusAnchor {
            blob_id,
            blake3_hex,
            byte_len,
        })
    }

    /// Store a Gaussian splat file, optionally compressing it to SOG format
    /// first if `splattransform` is present in PATH.
    ///
    /// 1. Reads the raw PLY bytes and computes their BLAKE3 (always, for audit).
    /// 2. Attempts `splattransform --input … --output … --format sog`.
    ///    If the binary is absent or the conversion fails, falls back to raw PLY.
    /// 3. Uploads whichever bytes are available via `store_blob`.
    /// 4. Returns a [`SplatAnchor`] with provenance fields.
    pub async fn store_splat(&self, ply_path: &Path, label: &str) -> Result<SplatAnchor, String> {
        // ── 1. Read original PLY ──────────────────────────────────────────────
        let ply_bytes = std::fs::read(ply_path)
            .map_err(|e| format!("store_splat: cannot read PLY {}: {e}", ply_path.display()))?;
        let ply_blake3 = blake3::hash(&ply_bytes).to_hex().to_string();
        let ply_len = ply_bytes.len();

        // ── 2. Try SOG compression via SplatTransform CLI ─────────────────────
        let sog_result: Option<Vec<u8>> = (|| -> Option<Vec<u8>> {
            // Check that splattransform is reachable before creating a temp file.
            let probe = std::process::Command::new("splattransform")
                .arg("--version")
                .output()
                .ok()?;
            if !probe.status.success() && probe.status.code() != Some(0) {
                // Some CLIs return non-zero for --version; treat any execution as present.
            }
            let tmp_dir = std::env::temp_dir();
            let stem = ply_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("splat");
            let tmp_sog = tmp_dir.join(format!("{stem}.sog"));
            let status = std::process::Command::new("splattransform")
                .args([
                    "--input",
                    &ply_path.display().to_string(),
                    "--output",
                    &tmp_sog.display().to_string(),
                    "--format",
                    "sog",
                ])
                .status()
                .ok()?;
            if !status.success() {
                return None;
            }
            let sog_bytes = std::fs::read(&tmp_sog).ok()?;
            let _ = std::fs::remove_file(&tmp_sog); // clean up temp
            Some(sog_bytes)
        })();

        // ── 3. Choose bytes and format ────────────────────────────────────────
        let (upload_bytes, format, compression_ratio) = match sog_result {
            Some(sog) => {
                let ratio = sog.len() as f32 / ply_len.max(1) as f32;
                (sog, SplatFormat::Sog, ratio)
            }
            None => (ply_bytes, SplatFormat::Ply, 1.0_f32),
        };

        // ── 4. Upload ─────────────────────────────────────────────────────────
        let walrus = self.store_blob(upload_bytes).await?;
        let _ = label; // used in receipt_payload; suppress unused-var lint
        Ok(SplatAnchor {
            walrus,
            format,
            compression_ratio,
            ply_blake3,
        })
    }

    /// Read a blob back from the aggregator.
    pub async fn read_blob(&self, blob_id: &str) -> Result<Vec<u8>, String> {
        let url = format!(
            "{}/v1/blobs/{}",
            self.config.aggregator_url.trim_end_matches('/'),
            urlencoding::encode(blob_id)
        );
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("walrus read failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("walrus read returned {}", resp.status()));
        }
        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| format!("walrus read: body error: {e}"))
    }

    /// Verify a fetched blob against its anchor.
    pub fn verify(anchor: &WalrusAnchor, bytes: &[u8]) -> bool {
        bytes.len() == anchor.byte_len
            && blake3::hash(bytes).to_hex().to_string() == anchor.blake3_hex
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::memdir::OduEntry;

    #[test]
    fn parse_newly_created_response() {
        let v = serde_json::json!({
            "newlyCreated": {
                "blobObject": {"id": "0xabc", "blobId": "b64blobid", "size": 42}
            }
        });
        assert_eq!(parse_store_response(&v).as_deref(), Some("b64blobid"));
    }

    #[test]
    fn parse_already_certified_response() {
        let v = serde_json::json!({
            "alreadyCertified": {"blobId": "existing-id", "endEpoch": 99}
        });
        assert_eq!(parse_store_response(&v).as_deref(), Some("existing-id"));
    }

    #[test]
    fn parse_rejects_unknown_shape() {
        assert!(parse_store_response(&serde_json::json!({"error": "nope"})).is_none());
    }

    #[test]
    fn export_fold_json_round_trips_and_hashes() {
        let mut dir = OduDirectory::new();
        let mut macro_node = OduEntry::new("rem:topics/x:100", "[REM fold] 2 entries", "topics/x");
        macro_node.tags.push("rem-fold".to_string());
        dir.insert(macro_node);
        dir.archive_fold(
            "rem:topics/x:100",
            vec![
                OduEntry::new("a", "first", "topics/x"),
                OduEntry::new("b", "second", "topics/x"),
            ],
        );

        let (json, hash) = export_fold_json(&dir, "rem:topics/x:100").expect("fold exists");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["kind"], "rem_fold_archive");
        assert_eq!(parsed["entries"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["macro_node"]["id"], "rem:topics/x:100");
        assert_eq!(hash, blake3::hash(json.as_bytes()).to_hex().to_string());

        assert!(export_fold_json(&dir, "rem:missing:0").is_none());
    }

    #[test]
    fn anchor_receipt_payload_carries_hash_not_data() {
        let anchor = WalrusAnchor {
            blob_id: "blob-1".to_string(),
            blake3_hex: "aa".repeat(32),
            byte_len: 1024,
        };
        let payload: serde_json::Value =
            serde_json::from_str(&anchor.receipt_payload("rem:topics/x:100")).unwrap();
        assert_eq!(payload["kind"], "walrus_anchor");
        assert_eq!(payload["blob_id"], "blob-1");
        assert_eq!(payload["byte_len"], 1024);
        assert!(payload.get("entries").is_none(), "never the data itself");
    }

    #[test]
    fn verify_checks_hash_and_length() {
        let bytes = b"hello walrus".to_vec();
        let anchor = WalrusAnchor {
            blob_id: "x".to_string(),
            blake3_hex: blake3::hash(&bytes).to_hex().to_string(),
            byte_len: bytes.len(),
        };
        assert!(WalrusClient::verify(&anchor, &bytes));
        assert!(!WalrusClient::verify(&anchor, b"tampered"));
    }

    #[test]
    fn config_from_env_requires_both_urls() {
        // Not set in the test environment → not configured.
        std::env::remove_var("WALRUS_PUBLISHER_URL");
        std::env::remove_var("WALRUS_AGGREGATOR_URL");
        assert!(WalrusConfig::from_env().is_none());
    }

    // ── SplatAnchor / SplatFormat tests ──────────────────────────────────────

    #[test]
    fn splat_format_serializes_correctly() {
        let sog_json = serde_json::to_string(&SplatFormat::Sog).unwrap();
        let ply_json = serde_json::to_string(&SplatFormat::Ply).unwrap();
        assert_eq!(sog_json, "\"Sog\"");
        assert_eq!(ply_json, "\"Ply\"");
        let rt: SplatFormat = serde_json::from_str(&sog_json).unwrap();
        assert_eq!(rt, SplatFormat::Sog);
    }

    #[test]
    fn splat_anchor_receipt_payload_structure() {
        let anchor = SplatAnchor {
            walrus: WalrusAnchor {
                blob_id: "sog-blob-1".to_string(),
                blake3_hex: "bb".repeat(32),
                byte_len: 512,
            },
            format: SplatFormat::Sog,
            compression_ratio: 0.42,
            ply_blake3: "cc".repeat(32),
        };
        let payload: serde_json::Value =
            serde_json::from_str(&anchor.receipt_payload("splat:scene_001")).unwrap();
        assert_eq!(payload["kind"], "splat_anchor");
        assert_eq!(payload["blob_id"], "sog-blob-1");
        assert_eq!(payload["format"], "sog");
        assert_eq!(payload["ply_blake3"], "cc".repeat(32));
        // compression_ratio must be present and numeric
        assert!(payload["compression_ratio"].is_number());
        // raw data must never appear
        assert!(payload.get("bytes").is_none());
    }

    #[test]
    fn splat_anchor_ply_blake3_is_hash_of_ply_not_sog() {
        // Simulate: original PLY bytes and (smaller) SOG bytes are different.
        let ply_bytes = b"PLY point cloud data here";
        let sog_bytes = b"SOG compressed";

        let ply_hash = blake3::hash(ply_bytes).to_hex().to_string();
        let sog_hash = blake3::hash(sog_bytes).to_hex().to_string();
        assert_ne!(ply_hash, sog_hash);

        // SplatAnchor stores the SOG blob in walrus but records the PLY hash.
        let anchor = SplatAnchor {
            walrus: WalrusAnchor {
                blob_id: "sog-blob".to_string(),
                blake3_hex: sog_hash.clone(), // walrus stores the sog hash
                byte_len: sog_bytes.len(),
            },
            format: SplatFormat::Sog,
            compression_ratio: sog_bytes.len() as f32 / ply_bytes.len() as f32,
            ply_blake3: ply_hash.clone(),
        };

        // Verify ply_blake3 matches ply, not sog.
        assert_eq!(anchor.ply_blake3, ply_hash);
        assert_ne!(anchor.ply_blake3, sog_hash);

        // And the receipt carries the ply hash for audit.
        let payload: serde_json::Value =
            serde_json::from_str(&anchor.receipt_payload("splat:test")).unwrap();
        assert_eq!(payload["ply_blake3"].as_str().unwrap(), ply_hash);
        assert_ne!(payload["ply_blake3"].as_str().unwrap(), sog_hash);
    }

    #[test]
    fn walrus_anchor_splat_label_prefix() {
        assert_eq!(WalrusAnchor::SPLAT_LABEL_PREFIX, "splat:");
    }
}
