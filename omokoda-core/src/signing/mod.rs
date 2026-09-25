/// Phase 18.2 — NIP-46 compatible remote signing integration.
///
/// The private key never leaves the identity vault.  External callers (agent
/// applications, DIP envelopes, L1 transactions) request signatures through
/// this signing module rather than accessing the raw key directly.
///
/// Supported payload kinds:
///   - NostrEvent   — signs a Nostr event (kind + content + tags)
///   - DipEnvelope  — signs a DIP envelope canonical_hash
///   - L1Tx         — signs an Ọ̀ṢỌ́ L1 transaction
///   - ArpReceipt   — signs an ARP receipt hash
///   - Raw          — signs arbitrary bytes (for internal use only)

pub mod request;
pub mod vault_signer;

pub use request::{SigningRequest, SigningPayload, SigningResponse};
pub use vault_signer::VaultSigner;
