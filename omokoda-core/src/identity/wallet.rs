use blake2::digest::consts::U32;
use blake2::{Blake2b, Digest};
use ed25519_dalek::SigningKey;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Sha512};
use sha3::{Keccak256, Sha3_256};

type Blake2b256 = Blake2b<U32>;

/// BIP-39 mnemonic -> 64-byte seed (standard PBKDF2-HMAC-SHA512, 2048
/// rounds, salt = "mnemonic" + passphrase). Shared by every chain's
/// derivation below -- they all fan out from the same root seed, only the
/// curve/path/child-derivation math differs per chain.
fn mnemonic_to_seed(mnemonic: &str, passphrase: &str) -> [u8; 64] {
    let mut seed = [0u8; 64];
    let salt = format!("mnemonic{}", passphrase);
    pbkdf2::pbkdf2::<Hmac<Sha512>>(mnemonic.as_bytes(), salt.as_bytes(), 2048, &mut seed)
        .expect("PBKDF2 failed");
    seed
}

/// One derived child chain's key material, hex-encoded for sealing into
/// `PrivateSessionData` alongside the existing Sui wallet key.
#[derive(Debug, Clone)]
pub struct ChainKey {
    pub private_key_hex: String,
    pub address: String,
}

/// EIP-55 mixed-case checksum for an Ethereum address.
/// `addr_hex` is the 40-char lowercase hex WITHOUT the "0x" prefix.
/// Ported from vanity-cloakseed's `crypto.ts::toChecksumAddress`.
pub fn eip55_checksum(addr_hex: &str) -> String {
    let mut hasher = Keccak256::new();
    hasher.update(addr_hex.as_bytes());
    let digest = hasher.finalize();
    addr_hex
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if c.is_ascii_alphabetic() {
                let nibble = (digest[i / 2] >> (if i % 2 == 0 { 4 } else { 0 })) & 0xf;
                if nibble >= 8 {
                    c.to_ascii_uppercase()
                } else {
                    c
                }
            } else {
                c
            }
        })
        .collect()
}

/// Ethereum: secp256k1 BIP-32, path m/44'/60'/0'/0 (ported verbatim from
/// vanity-cloakseed's `chains.ts` CHAINS.ethereum.bip44), address = EIP-55
/// checksummed last-20-bytes of keccak256(uncompressed pubkey[1..]).
pub fn derive_ethereum(mnemonic: &str, passphrase: &str) -> Result<ChainKey, String> {
    let seed = mnemonic_to_seed(mnemonic, passphrase);
    let xprv = bip32::XPrv::derive_from_path(
        seed,
        &"m/44'/60'/0'/0"
            .parse::<bip32::DerivationPath>()
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let signing_key: k256::ecdsa::SigningKey = xprv.private_key().clone();
    let verifying_key = signing_key.verifying_key();
    let uncompressed = verifying_key.to_encoded_point(false);
    let pubkey_bytes = uncompressed.as_bytes(); // 0x04 || X(32) || Y(32)
    let mut hasher = Keccak256::new();
    hasher.update(&pubkey_bytes[1..]);
    let digest = hasher.finalize();
    let addr_hex = hex::encode(&digest[12..]);
    let address = format!("0x{}", eip55_checksum(&addr_hex));
    Ok(ChainKey {
        private_key_hex: hex::encode(signing_key.to_bytes()),
        address,
    })
}

/// Bitcoin: secp256k1 BIP-32, path m/84'/0'/0'/0 (native segwit account
/// root, matching vanity-cloakseed's chains.ts CHAINS.bitcoin.bip44).
/// Address = bech32 P2WPKH (bc1...) of hash160(compressed pubkey).
pub fn derive_bitcoin(mnemonic: &str, passphrase: &str) -> Result<ChainKey, String> {
    let seed = mnemonic_to_seed(mnemonic, passphrase);
    let xprv = bip32::XPrv::derive_from_path(
        seed,
        &"m/84'/0'/0'/0"
            .parse::<bip32::DerivationPath>()
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let signing_key: k256::ecdsa::SigningKey = xprv.private_key().clone();
    let verifying_key = signing_key.verifying_key();
    let compressed = verifying_key.to_encoded_point(true);
    let sha = Sha256::digest(compressed.as_bytes());
    let hash160 = ripemd::Ripemd160::digest(sha);
    let address = bech32_p2wpkh("bc", &hash160)?;
    Ok(ChainKey {
        private_key_hex: hex::encode(signing_key.to_bytes()),
        address,
    })
}

/// Cosmos Hub: secp256k1 BIP-32, path m/44'/118'/0'/0 (matching
/// vanity-cloakseed's chains.ts CHAINS.cosmos.bip44). Address = bech32
/// "cosmos" of hash160(compressed pubkey).
pub fn derive_cosmos(mnemonic: &str, passphrase: &str) -> Result<ChainKey, String> {
    let seed = mnemonic_to_seed(mnemonic, passphrase);
    let xprv = bip32::XPrv::derive_from_path(
        seed,
        &"m/44'/118'/0'/0"
            .parse::<bip32::DerivationPath>()
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let signing_key: k256::ecdsa::SigningKey = xprv.private_key().clone();
    let verifying_key = signing_key.verifying_key();
    let compressed = verifying_key.to_encoded_point(true);
    let sha = Sha256::digest(compressed.as_bytes());
    let hash160 = ripemd::Ripemd160::digest(sha);
    let address = bech32_addr("cosmos", &hash160)?;
    Ok(ChainKey {
        private_key_hex: hex::encode(signing_key.to_bytes()),
        address,
    })
}

/// Solana: Ed25519 SLIP-0010, path m/44'/501'/0'/0' (matching
/// vanity-cloakseed's chains.ts CHAINS.solana.bip44 -- every segment
/// hardened, as SLIP-0010 ed25519 requires). Address = base58(pubkey).
pub fn derive_solana(mnemonic: &str, passphrase: &str) -> Result<ChainKey, String> {
    let seed = mnemonic_to_seed(mnemonic, passphrase);
    let path = [44 | HARDENED, 501 | HARDENED, 0 | HARDENED, 0 | HARDENED];
    let signing_key = Wallet::derive_ed25519_slip10(&seed, &path)?;
    let pubkey = signing_key.verifying_key().to_bytes();
    Ok(ChainKey {
        private_key_hex: hex::encode(signing_key.to_bytes()),
        address: bs58::encode(pubkey).into_string(),
    })
}

/// Aptos: Ed25519 SLIP-0010, path m/44'/637'/0'/0' (matching
/// vanity-cloakseed's chains.ts CHAINS.aptos.bip44). Address =
/// sha3-256(pubkey || 0x00) -- Aptos's single-signer Ed25519 scheme.
pub fn derive_aptos(mnemonic: &str, passphrase: &str) -> Result<ChainKey, String> {
    let seed = mnemonic_to_seed(mnemonic, passphrase);
    let path = [44 | HARDENED, 637 | HARDENED, 0 | HARDENED, 0 | HARDENED];
    let signing_key = Wallet::derive_ed25519_slip10(&seed, &path)?;
    let pubkey = signing_key.verifying_key().to_bytes();
    let mut hasher = Sha3_256::new();
    hasher.update(pubkey);
    hasher.update([0x00]); // Ed25519 single-signer scheme identifier
    let digest = hasher.finalize();
    Ok(ChainKey {
        private_key_hex: hex::encode(signing_key.to_bytes()),
        address: format!("0x{}", hex::encode(digest)),
    })
}

/// Nostr (NIP-06): Ed25519 SLIP-0010, path m/44'/1237'/<account>'/0/0.
/// Every segment is derived hardened here (the same treatment the existing
/// Sui derivation already gives m/44'/784'/0'/0'/0' above) since SLIP-0010
/// ed25519 has no defined non-hardened child derivation -- there is no
/// public-key-only path to derive from for Ed25519. Address = bech32
/// "npub" of the raw pubkey (NIP-19).
pub fn derive_nostr(mnemonic: &str, passphrase: &str, account: u32) -> Result<ChainKey, String> {
    let seed = mnemonic_to_seed(mnemonic, passphrase);
    let path = [44 | HARDENED, 1237 | HARDENED, account | HARDENED, 0, 0];
    let signing_key = Wallet::derive_ed25519_slip10(&seed, &path)?;
    let pubkey = signing_key.verifying_key().to_bytes();
    let address = bech32_npub(&pubkey)?;
    Ok(ChainKey {
        private_key_hex: hex::encode(signing_key.to_bytes()),
        address,
    })
}

/// minipae NIP-AE agent key: secp256k1 BIP-32, path m/44'/30174'/<agent_index>'/<owner_index>'
/// (hardened only), matching minipae's own derive.py exactly (30174' is the
/// NIP-AE application constant, same root-derivation scheme as Ethereum's
/// m/44'/60'/0'/0 above -- purpose/application/agent/owner instead of
/// purpose/coin/account/change). This is a sibling derivation from the same
/// BIPON39 mnemonic every agent already gets at birth, not a second
/// unrelated key: minipae never needs to see the mnemonic, only this leaf.
/// agent_index/owner_index must each be < 2^31 (BIP-32 hardened-index limit);
/// callers derive them from the agent's own identifier (e.g. first 4 bytes
/// of sha256(agent_name) masked to 31 bits), owner_index=0 for self-owned.
/// Address = bech32 npub of the x-only (BIP-340) pubkey, matching minipae's
/// own NIP-AE author-field convention.
pub fn derive_minipae_key(
    mnemonic: &str,
    passphrase: &str,
    agent_index: u32,
    owner_index: u32,
) -> Result<ChainKey, String> {
    if agent_index >= HARDENED || owner_index >= HARDENED {
        return Err("agent_index/owner_index must be < 2^31".into());
    }
    let seed = mnemonic_to_seed(mnemonic, passphrase);
    let path = format!("m/44'/30174'/{}'/{}'", agent_index, owner_index);
    let xprv = bip32::XPrv::derive_from_path(
        seed,
        &path
            .parse::<bip32::DerivationPath>()
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let signing_key: k256::ecdsa::SigningKey = xprv.private_key().clone();
    let verifying_key = signing_key.verifying_key();
    let compressed = verifying_key.to_encoded_point(true);
    // BIP-340/Nostr x-only pubkey: drop the parity-flag byte, keep the X coordinate.
    let x_only: [u8; 32] = compressed.as_bytes()[1..33]
        .try_into()
        .map_err(|_| "bad pubkey length")?;
    let address = bech32_npub(&x_only)?;
    Ok(ChainKey {
        private_key_hex: hex::encode(signing_key.to_bytes()),
        address,
    })
}

/// Deterministic BIP-32 hardened index (< 2^31) for a minipae agent/owner
/// identifier string -- first 4 bytes of sha256, masked to 31 bits.
pub fn minipae_index_for(id: &str) -> u32 {
    let digest = Sha256::digest(id.as_bytes());
    u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) & 0x7fff_ffff
}

const HARDENED: u32 = 0x8000_0000;

/// Derive libp2p Ed25519 peer ID from k_root.
///
/// Uses a distinct HMAC path so the resulting keypair never collides with the
/// Nostr key or any chain key.  The "peer_id" returned here is the hex-encoded
/// 32-byte Ed25519 public key — a lightweight representation that does not
/// require the libp2p crate (which is not in scope for omokoda-core).  Full
/// multiaddr / multihash encoding can be added by the networking layer once the
/// libp2p crate is wired in.
///
/// Path mnemonic: "omokoda:libp2p:v1"
pub fn derive_libp2p_key(k_root: &[u8]) -> (String, String) {
    let mut mac = Hmac::<Sha256>::new_from_slice(k_root).expect("HMAC accepts any key length");
    mac.update(b"omokoda:libp2p:v1");
    let seed: [u8; 32] = mac.finalize().into_bytes().into();
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
    let pubkey = signing_key.verifying_key().to_bytes();
    (hex::encode(seed), hex::encode(pubkey))
}

/// Derive the deterministic email local-part for an agent from k_root.
///
/// Returns the first 16 hex characters of HMAC-SHA256(k_root,
/// "omokoda:email:v1") — 8 bytes of entropy, short enough for a local-part
/// while still globally unique across the agent population.
///
/// Path mnemonic: "omokoda:email:v1"
pub fn derive_email_local(k_root: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(k_root).expect("HMAC accepts any key length");
    mac.update(b"omokoda:email:v1");
    let result = mac.finalize().into_bytes();
    hex::encode(&result[..8]) // 8 bytes → 16 hex chars
}

fn bech32_p2wpkh(hrp_str: &str, hash160: &[u8]) -> Result<String, String> {
    use bech32::Hrp;
    let hrp = Hrp::parse(hrp_str).map_err(|e| e.to_string())?;
    bech32::segwit::encode_v0(hrp, hash160).map_err(|e| e.to_string())
}

fn bech32_addr(hrp_str: &str, hash160: &[u8]) -> Result<String, String> {
    use bech32::{Bech32, Hrp};
    let hrp = Hrp::parse(hrp_str).map_err(|e| e.to_string())?;
    bech32::encode::<Bech32>(hrp, hash160).map_err(|e| e.to_string())
}

fn bech32_npub(pubkey: &[u8; 32]) -> Result<String, String> {
    use bech32::{Bech32, Hrp};
    let hrp = Hrp::parse("npub").map_err(|e| e.to_string())?;
    bech32::encode::<Bech32>(hrp, pubkey).map_err(|e| e.to_string())
}

/// Derive the real Sui address from an Ed25519 public key: `0x` + hex of
/// `blake2b256(flag_byte || pubkey)`, `flag_byte = 0x00` for the Ed25519
/// signature scheme -- Sui's actual on-chain address format (SIP-6), not the
/// raw public key hex that was published here before. Ported from
/// vanity-cloakseed's `chainCrypto.ts::deriveSuiAddress`, verified against
/// the same algorithm.
pub fn sui_address_from_pubkey(pubkey: &[u8; 32]) -> String {
    let mut hasher = Blake2b256::new();
    hasher.update([0x00]);
    hasher.update(pubkey);
    let digest = hasher.finalize();
    format!("0x{}", hex::encode(digest))
}

pub struct Wallet;

impl Wallet {
    /// Derives an Ed25519 keypair from a mnemonic using SLIP-0010 (m/44'/784'/0'/0'/0')
    /// for Sui compatibility.
    pub fn derive_from_mnemonic(mnemonic: &str, passphrase: &str) -> Result<SigningKey, String> {
        // 1. Mnemonic to seed (BIP-39 standard seed derivation for the master key)
        // Note: The architecture spec mentions argon2id for "identity-critical" seeds,
        // but for a standard Sui wallet compatibility, BIP-39 PBKDF2 is usually expected
        // if interacting with other wallets.
        // However, the architecture says "Sui wallet — Ed25519 keypair, m/44'/784' derivation from mnemonic".
        // Let's use the PBKDF2 seed for the BIP-39 master seed, then SLIP-0010 for derivation.

        let seed = mnemonic_to_seed(mnemonic, passphrase);
        Self::derive_from_seed(&seed)
    }

    pub fn derive_from_seed(seed: &[u8; 64]) -> Result<SigningKey, String> {
        // Derivation path: m/44'/784'/0'/0'/0'
        // Every step is hardened for Ed25519 as per SLIP-0010.
        let path = [
            44 | 0x8000_0000,
            784 | 0x8000_0000,
            0x8000_0000,
            0x8000_0000,
            0x8000_0000,
        ];
        Self::derive_ed25519_slip10(seed, &path)
    }

    /// General-purpose SLIP-0010 Ed25519 derivation used by every ed25519
    /// chain here (Sui, Solana, Aptos, Nostr) -- only the path differs per
    /// chain, the master-key + child-key math is identical.
    pub fn derive_ed25519_slip10(seed: &[u8; 64], path: &[u32]) -> Result<SigningKey, String> {
        // SLIP-0010 master key derivation
        let mut hmac =
            Hmac::<Sha512>::new_from_slice(b"ed25519 seed").map_err(|e| e.to_string())?;
        hmac.update(seed);
        let intermediate = hmac.finalize().into_bytes();

        let mut il = [0u8; 32];
        let mut ir = [0u8; 32];
        il.copy_from_slice(&intermediate[..32]);
        ir.copy_from_slice(&intermediate[32..]);

        let (mut kl, mut kr) = (il, ir);
        for &index in path {
            (kl, kr) = Self::derive_child(kl, kr, index)?;
        }

        Ok(SigningKey::from_bytes(&kl))
    }

    fn derive_child(
        kl: [u8; 32],
        kr: [u8; 32],
        index: u32,
    ) -> Result<([u8; 32], [u8; 32]), String> {
        let mut hmac = Hmac::<Sha512>::new_from_slice(&kr).map_err(|e| e.to_string())?;
        hmac.update(&[0u8]); // hardened indicator for SLIP-0010
        hmac.update(&kl);
        hmac.update(&index.to_be_bytes());
        let intermediate = hmac.finalize().into_bytes();

        let mut il = [0u8; 32];
        let mut ir = [0u8; 32];
        il.copy_from_slice(&intermediate[..32]);
        ir.copy_from_slice(&intermediate[32..]);
        Ok((il, ir))
    }
}

// ── Vanity address mining ──────────────────────────────────────────────────

/// Result of a vanity address mining run.
#[derive(Debug, Clone)]
pub struct VanityResult {
    pub private_key_hex: String,
    pub public_key_hex: String,
    pub address: String,
    pub attempts: u64,
    pub duration_ms: u128,
}

/// Expected number of attempts to find a match (16^n for ETH hex, 58^n for base58).
pub fn estimate_vanity_attempts(prefix: &str, suffix: &str, chain: &str) -> f64 {
    let len = (prefix.len() + suffix.len()) as u32;
    if len == 0 {
        return 1.0;
    }
    match chain {
        "sol" | "solana" | "btc" | "bitcoin" => 58f64.powi(len as i32),
        _ => 16f64.powi(len as i32),
    }
}

/// Mine a vanity Ethereum address (secp256k1, EIP-55 checksummed).
///
/// Generates random private keys until the lowercase hex address (without `0x`)
/// starts with `prefix` and ends with `suffix`. Both comparisons are
/// case-insensitive on the hex nibbles.
///
/// Returns `Err` if `max_attempts` is exhausted without a match.
pub fn mine_eth_vanity(
    prefix: &str,
    suffix: &str,
    max_attempts: u64,
) -> Result<VanityResult, String> {
    use k256::ecdsa::SigningKey as K256SigningKey;
    use rand::RngCore;

    let prefix_lc = prefix.to_lowercase();
    let suffix_lc = suffix.to_lowercase();
    if prefix_lc.chars().any(|c| !c.is_ascii_hexdigit())
        || suffix_lc.chars().any(|c| !c.is_ascii_hexdigit())
    {
        return Err("Ethereum vanity pattern must contain only hex characters 0-9 a-f".into());
    }
    if prefix_lc.len() + suffix_lc.len() > 40 {
        return Err("Combined vanity pattern length exceeds 40 hex chars".into());
    }

    let mut rng = rand::thread_rng();
    let start = std::time::Instant::now();

    for attempt in 0..max_attempts {
        let mut raw = [0u8; 32];
        rng.fill_bytes(&mut raw);
        let Ok(signing_key) = K256SigningKey::from_bytes(raw.as_slice().into()) else {
            continue;
        };
        let verifying = signing_key.verifying_key();
        // Uncompressed public key = 0x04 || X || Y (65 bytes); skip the 0x04 prefix.
        let pubkey_bytes = verifying.to_encoded_point(false);
        let pub_uncompressed = &pubkey_bytes.as_bytes()[1..]; // 64 bytes
        let mut hasher = Keccak256::new();
        hasher.update(pub_uncompressed);
        let digest = hasher.finalize();
        let addr_hex = hex::encode(&digest[12..]); // last 20 bytes = 40 hex chars

        let matches = (prefix_lc.is_empty() || addr_hex.starts_with(prefix_lc.as_str()))
            && (suffix_lc.is_empty() || addr_hex.ends_with(suffix_lc.as_str()));

        if matches {
            return Ok(VanityResult {
                private_key_hex: hex::encode(signing_key.to_bytes()),
                public_key_hex: hex::encode(pubkey_bytes.as_bytes()),
                address: format!("0x{}", eip55_checksum(&addr_hex)),
                attempts: attempt + 1,
                duration_ms: start.elapsed().as_millis(),
            });
        }
    }

    Err(format!(
        "Vanity mine exhausted after {max_attempts} attempts without finding \
         a match for prefix={prefix_lc:?} suffix={suffix_lc:?}"
    ))
}

/// Mine a vanity Solana address (Ed25519, base58-encoded).
///
/// Generates random Ed25519 private keys until the base58 address starts with
/// `prefix` and ends with `suffix` (case-sensitive — base58 is case-sensitive).
pub fn mine_sol_vanity(
    prefix: &str,
    suffix: &str,
    max_attempts: u64,
) -> Result<VanityResult, String> {
    use rand::RngCore;

    if prefix.len() + suffix.len() > 44 {
        return Err("Combined Solana vanity pattern length exceeds 44 chars".into());
    }

    let mut rng = rand::thread_rng();
    let start = std::time::Instant::now();

    for attempt in 0..max_attempts {
        let mut raw = [0u8; 32];
        rng.fill_bytes(&mut raw);
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&raw);
        let verifying_bytes = signing_key.verifying_key().to_bytes();
        let addr = bs58::encode(&verifying_bytes).into_string();

        let matches = (prefix.is_empty() || addr.starts_with(prefix))
            && (suffix.is_empty() || addr.ends_with(suffix));

        if matches {
            return Ok(VanityResult {
                private_key_hex: hex::encode(raw),
                public_key_hex: hex::encode(verifying_bytes),
                address: addr,
                attempts: attempt + 1,
                duration_ms: start.elapsed().as_millis(),
            });
        }
    }

    Err(format!(
        "Vanity mine exhausted after {max_attempts} attempts without finding \
         a match for prefix={prefix:?} suffix={suffix:?}"
    ))
}

// ── CREATE2 vanity contract address miner ──────────────────────────────────

/// Result of a CREATE2 vanity salt search.
#[derive(Debug, Clone)]
pub struct Create2VanityResult {
    /// 32-byte random salt as lowercase hex (no 0x prefix).
    pub salt_hex: String,
    /// EIP-55 checksummed `0x…` contract address.
    pub contract_address: String,
    pub attempts: u64,
    pub duration_ms: u128,
}

/// Compute the CREATE2 deployment address given a deployer, salt, and bytecode.
///
/// All inputs may optionally have a `0x` prefix.
/// Returns an EIP-55 checksummed `0x…` address.
pub fn create2_address(
    deployer_hex: &str,
    salt_hex: &str,
    bytecode_hex: &str,
) -> Result<String, String> {
    // Strip 0x prefixes.
    let deployer = deployer_hex.trim_start_matches("0x");
    let salt = salt_hex.trim_start_matches("0x");
    let bytecode = bytecode_hex.trim_start_matches("0x");

    if deployer.len() != 40 {
        return Err(format!(
            "deployer must be 40 hex chars (20 bytes), got {}",
            deployer.len()
        ));
    }
    if !deployer.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("deployer contains non-hex characters".into());
    }

    // Decode deployer (20 bytes).
    let deployer_bytes = hex::decode(deployer).map_err(|e| e.to_string())?;

    // Decode and left-pad salt to 32 bytes.
    let salt_raw = hex::decode(salt).map_err(|e| e.to_string())?;
    if salt_raw.len() > 32 {
        return Err("salt must be at most 32 bytes".into());
    }
    let mut salt_bytes = [0u8; 32];
    salt_bytes[32 - salt_raw.len()..].copy_from_slice(&salt_raw);

    // keccak256(bytecode).
    let bytecode_bytes = hex::decode(bytecode).map_err(|e| e.to_string())?;
    let mut hasher = Keccak256::new();
    hasher.update(&bytecode_bytes);
    let init_code_hash = hasher.finalize();

    // Preimage: 0xff ++ deployer(20) ++ salt(32) ++ keccak256(bytecode)(32) = 85 bytes.
    let mut preimage = [0u8; 85];
    preimage[0] = 0xff;
    preimage[1..21].copy_from_slice(&deployer_bytes);
    preimage[21..53].copy_from_slice(&salt_bytes);
    preimage[53..85].copy_from_slice(&init_code_hash);

    // Hash the preimage; take last 20 bytes.
    let mut hasher2 = Keccak256::new();
    hasher2.update(&preimage);
    let digest = hasher2.finalize();
    let addr_hex = hex::encode(&digest[12..]);

    Ok(format!("0x{}", eip55_checksum(&addr_hex)))
}

/// Mine a random salt such that the CREATE2 deployment address starts with
/// `prefix` and ends with `suffix` (case-insensitive hex, without `0x`).
///
/// Returns `Err` if `max_attempts` is exhausted without a match.
pub fn mine_create2_vanity(
    deployer_hex: &str,
    bytecode_hex: &str,
    prefix: &str,
    suffix: &str,
    max_attempts: u64,
) -> Result<Create2VanityResult, String> {
    use rand::RngCore;

    let prefix_lc = prefix.to_lowercase();
    let suffix_lc = suffix.to_lowercase();

    if prefix_lc.chars().any(|c| !c.is_ascii_hexdigit())
        || suffix_lc.chars().any(|c| !c.is_ascii_hexdigit())
    {
        return Err("CREATE2 vanity pattern must contain only hex characters 0-9 a-f".into());
    }
    if prefix_lc.len() + suffix_lc.len() > 40 {
        return Err("Combined vanity pattern length exceeds 40 hex chars".into());
    }

    // Pre-validate deployer and compute bytecode hash once.
    let deployer = deployer_hex.trim_start_matches("0x");
    if deployer.len() != 40 || !deployer.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("deployer must be 40 valid hex chars".into());
    }
    let deployer_bytes = hex::decode(deployer).map_err(|e| e.to_string())?;

    let bytecode_raw =
        hex::decode(bytecode_hex.trim_start_matches("0x")).map_err(|e| e.to_string())?;
    let mut bh = Keccak256::new();
    bh.update(&bytecode_raw);
    let init_code_hash = bh.finalize();

    let mut rng = rand::thread_rng();
    let start = std::time::Instant::now();

    for attempt in 0..max_attempts {
        let mut salt_bytes = [0u8; 32];
        rng.fill_bytes(&mut salt_bytes);

        // Build preimage inline to avoid per-iteration heap alloc.
        let mut preimage = [0u8; 85];
        preimage[0] = 0xff;
        preimage[1..21].copy_from_slice(&deployer_bytes);
        preimage[21..53].copy_from_slice(&salt_bytes);
        preimage[53..85].copy_from_slice(&init_code_hash);

        let mut hasher = Keccak256::new();
        hasher.update(&preimage);
        let digest = hasher.finalize();
        let addr_hex = hex::encode(&digest[12..]);

        let matches = (prefix_lc.is_empty() || addr_hex.starts_with(prefix_lc.as_str()))
            && (suffix_lc.is_empty() || addr_hex.ends_with(suffix_lc.as_str()));

        if matches {
            return Ok(Create2VanityResult {
                salt_hex: hex::encode(salt_bytes),
                contract_address: format!("0x{}", eip55_checksum(&addr_hex)),
                attempts: attempt + 1,
                duration_ms: start.elapsed().as_millis(),
            });
        }
    }

    Err(format!(
        "CREATE2 vanity mine exhausted after {max_attempts} attempts without finding \
         a match for prefix={prefix_lc:?} suffix={suffix_lc:?}"
    ))
}

// ── EIP-2307 keystore v3 JSON export ──────────────────────────────────────

/// EIP-2307 / Web3 Secret Storage Definition (keystore v3) encrypted wallet.
///
/// Cipher used: `xchacha20poly1305` (AEAD).  The nonce is stored in
/// `cipherparams.nonce` (24 bytes hex).  MAC is
/// `keccak256(derived_key[16..32] ++ ciphertext)` matching the geth v3 MAC
/// convention.  KDF is PBKDF2-HMAC-SHA256 with 262 144 iterations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeystoreV3 {
    pub crypto: KeystoreCrypto,
    /// UUID v4.
    pub id: String,
    /// Always 3.
    pub version: u8,
    /// Lowercase hex without `0x`.
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeystoreCrypto {
    /// `"xchacha20poly1305"`
    pub cipher: String,
    /// Hex-encoded ciphertext (includes AEAD tag).
    pub ciphertext: String,
    /// `{"nonce": "<24-byte hex>"}`.
    pub cipherparams: serde_json::Value,
    /// `"pbkdf2"`
    pub kdf: String,
    /// `{"c": 262144, "dklen": 32, "prf": "hmac-sha256", "salt": "<hex>"}`.
    pub kdfparams: serde_json::Value,
    /// `keccak256(derived_key[16..32] ++ ciphertext)` as hex.
    pub mac: String,
}

/// Export an Ethereum private key as an encrypted EIP-2307 keystore v3 JSON.
///
/// `private_key_hex` may optionally have a `0x` prefix.
/// `password` is the user-supplied passphrase for encryption.
pub fn export_keystore_v3(private_key_hex: &str, password: &str) -> Result<KeystoreV3, String> {
    use chacha20poly1305::aead::{Aead, KeyInit as _};
    use chacha20poly1305::{Key as ChaChaKey, XChaCha20Poly1305, XNonce};
    use rand::RngCore;
    use uuid::Uuid;

    let pk_hex = private_key_hex.trim_start_matches("0x");
    if pk_hex.len() != 64 {
        return Err(format!(
            "private key must be 64 hex chars (32 bytes), got {}",
            pk_hex.len()
        ));
    }
    let pk_bytes = hex::decode(pk_hex).map_err(|e| e.to_string())?;

    // Derive Ethereum address from private key.
    let signing_key = k256::ecdsa::SigningKey::from_bytes(pk_bytes.as_slice().into())
        .map_err(|e| e.to_string())?;
    let verifying_key = signing_key.verifying_key();
    let uncompressed = verifying_key.to_encoded_point(false);
    let pub_bytes = uncompressed.as_bytes();
    let mut addr_hasher = Keccak256::new();
    addr_hasher.update(&pub_bytes[1..]);
    let addr_digest = addr_hasher.finalize();
    let address = hex::encode(&addr_digest[12..]);

    let mut rng = rand::thread_rng();

    // KDF: PBKDF2-HMAC-SHA256, 262144 rounds, 32-byte output.
    let mut salt = [0u8; 32];
    rng.fill_bytes(&mut salt);
    let mut derived_key = [0u8; 32];
    pbkdf2::pbkdf2::<Hmac<Sha256>>(password.as_bytes(), &salt, 262_144, &mut derived_key)
        .map_err(|e| format!("PBKDF2 error: {e}"))?;

    // Encrypt: XChaCha20Poly1305 with the first 32 bytes of derived key.
    let mut nonce_bytes = [0u8; 24];
    rng.fill_bytes(&mut nonce_bytes);
    let cipher_key = ChaChaKey::from_slice(&derived_key);
    let cipher = XChaCha20Poly1305::new(cipher_key);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, pk_bytes.as_slice())
        .map_err(|e| format!("encryption error: {e}"))?;

    // MAC = keccak256(derived_key[16..32] ++ ciphertext).
    let mut mac_hasher = Keccak256::new();
    mac_hasher.update(&derived_key[16..32]);
    mac_hasher.update(&ciphertext);
    let mac_digest = mac_hasher.finalize();

    Ok(KeystoreV3 {
        crypto: KeystoreCrypto {
            cipher: "xchacha20poly1305".into(),
            ciphertext: hex::encode(&ciphertext),
            cipherparams: serde_json::json!({ "nonce": hex::encode(nonce_bytes) }),
            kdf: "pbkdf2".into(),
            kdfparams: serde_json::json!({
                "c": 262_144u32,
                "dklen": 32u32,
                "prf": "hmac-sha256",
                "salt": hex::encode(salt),
            }),
            mac: hex::encode(mac_digest),
        },
        id: Uuid::new_v4().to_string(),
        version: 3,
        address,
    })
}

/// Decrypt a keystore v3 JSON and return the private key as lowercase hex
/// (no `0x` prefix).
pub fn import_keystore_v3(keystore: &KeystoreV3, password: &str) -> Result<String, String> {
    use chacha20poly1305::aead::{Aead, KeyInit as _};
    use chacha20poly1305::{Key as ChaChaKey, XChaCha20Poly1305, XNonce};

    let crypto = &keystore.crypto;

    if crypto.kdf != "pbkdf2" {
        return Err(format!("unsupported KDF: {}", crypto.kdf));
    }
    if crypto.cipher != "xchacha20poly1305" {
        return Err(format!("unsupported cipher: {}", crypto.cipher));
    }

    // Parse KDF params.
    let params = &crypto.kdfparams;
    let c = params["c"].as_u64().ok_or("missing kdfparams.c")? as u32;
    let salt_hex = params["salt"].as_str().ok_or("missing kdfparams.salt")?;
    let salt = hex::decode(salt_hex).map_err(|e| e.to_string())?;

    // Derive key.
    let mut derived_key = [0u8; 32];
    pbkdf2::pbkdf2::<Hmac<Sha256>>(password.as_bytes(), &salt, c, &mut derived_key)
        .map_err(|e| format!("PBKDF2 error: {e}"))?;

    // Decode ciphertext.
    let ciphertext = hex::decode(&crypto.ciphertext).map_err(|e| e.to_string())?;

    // Verify MAC.
    let mut mac_hasher = Keccak256::new();
    mac_hasher.update(&derived_key[16..32]);
    mac_hasher.update(&ciphertext);
    let mac_digest = mac_hasher.finalize();
    let expected_mac = hex::encode(mac_digest);
    if crypto.mac != expected_mac {
        return Err("MAC mismatch — wrong password or corrupted keystore".into());
    }

    // Decrypt.
    let nonce_hex = crypto.cipherparams["nonce"]
        .as_str()
        .ok_or("missing cipherparams.nonce")?;
    let nonce_bytes = hex::decode(nonce_hex).map_err(|e| e.to_string())?;
    if nonce_bytes.len() != 24 {
        return Err(format!("nonce must be 24 bytes, got {}", nonce_bytes.len()));
    }
    let cipher_key = ChaChaKey::from_slice(&derived_key);
    let cipher = XChaCha20Poly1305::new(cipher_key);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_slice())
        .map_err(|_| "decryption failed — wrong password or corrupted ciphertext")?;

    Ok(hex::encode(plaintext))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real vector: `sui keytool generate ed25519 --json` on this box printed
    /// suiAddress = 0x498724...71ba for peerId (raw pubkey hex)
    /// ff8790...fa32c -- independently verified with Python's hashlib
    /// (blake2b digest_size=32) before trusting it here. Not a synthetic
    /// vector; the actual `sui` binary computed this address.
    #[test]
    fn sui_address_matches_the_real_sui_cli() {
        let pubkey_hex = "ff879040047ab33258afade9e5505defc37e4d5dac2d770b702a526d40bfa32c";
        let pubkey: [u8; 32] = hex::decode(pubkey_hex).unwrap().try_into().unwrap();
        let address = sui_address_from_pubkey(&pubkey);
        assert_eq!(
            address,
            "0x498724481844b13ea6f8277c65af18774e03d5b81b6d40d4258cd8f12b2871ba"
        );
    }

    #[test]
    fn sui_address_is_deterministic_and_well_formed() {
        let pubkey = [7u8; 32];
        let a = sui_address_from_pubkey(&pubkey);
        let b = sui_address_from_pubkey(&pubkey);
        assert_eq!(a, b);
        assert!(a.starts_with("0x"));
        assert_eq!(a.len(), 66, "0x + 64 hex chars (32-byte digest)");
    }

    #[test]
    fn sui_address_differs_from_the_raw_pubkey_hex() {
        // The bug this replaces: publishing raw pubkey hex as if it were the
        // address. They must never be equal.
        let pubkey = [3u8; 32];
        let address = sui_address_from_pubkey(&pubkey);
        assert_ne!(address, format!("0x{}", hex::encode(pubkey)));
    }

    /// Standard BIP-39 test mnemonic (all-"abandon" + "about" checksum
    /// word) -- not a real identity, used purely as a fixed, reproducible
    /// input for the determinism/distinctness/byte-length checks below.
    const TEST_MNEMONIC: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    #[test]
    fn ethereum_key_is_deterministic_and_well_formed() {
        let a = derive_ethereum(TEST_MNEMONIC, "").unwrap();
        let b = derive_ethereum(TEST_MNEMONIC, "").unwrap();
        assert_eq!(a.private_key_hex, b.private_key_hex);
        assert_eq!(a.address, b.address);
        assert_eq!(a.private_key_hex.len(), 64, "32-byte secp256k1 scalar");
        assert!(a.address.starts_with("0x"));
        assert_eq!(a.address.len(), 42, "0x + 20-byte address");
        // EIP-55: re-lowercasing and re-checksumming must reproduce the same address.
        let addr_hex = &a.address[2..];
        let rechecked = format!("0x{}", eip55_checksum(&addr_hex.to_lowercase()));
        assert_eq!(a.address, rechecked, "EIP-55 checksum must be idempotent");
    }

    #[test]
    fn eip55_checksum_known_vector() {
        // EIP-55 spec example: https://eips.ethereum.org/EIPS/eip-55
        let checksummed = eip55_checksum("5aaeb6053f3e94c9b9a09f33669435e7ef1beaed");
        assert_eq!(checksummed, "5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed");
    }

    #[test]
    fn bitcoin_key_is_deterministic_and_well_formed() {
        let a = derive_bitcoin(TEST_MNEMONIC, "").unwrap();
        let b = derive_bitcoin(TEST_MNEMONIC, "").unwrap();
        assert_eq!(a.private_key_hex, b.private_key_hex);
        assert_eq!(a.address, b.address);
        assert_eq!(a.private_key_hex.len(), 64, "32-byte secp256k1 scalar");
        assert!(a.address.starts_with("bc1"), "native segwit P2WPKH address");
    }

    #[test]
    fn cosmos_key_is_deterministic_and_well_formed() {
        let a = derive_cosmos(TEST_MNEMONIC, "").unwrap();
        let b = derive_cosmos(TEST_MNEMONIC, "").unwrap();
        assert_eq!(a.private_key_hex, b.private_key_hex);
        assert_eq!(a.address, b.address);
        assert_eq!(a.private_key_hex.len(), 64, "32-byte secp256k1 scalar");
        assert!(a.address.starts_with("cosmos1"));
    }

    #[test]
    fn solana_key_is_deterministic_and_well_formed() {
        let a = derive_solana(TEST_MNEMONIC, "").unwrap();
        let b = derive_solana(TEST_MNEMONIC, "").unwrap();
        assert_eq!(a.private_key_hex, b.private_key_hex);
        assert_eq!(a.address, b.address);
        assert_eq!(a.private_key_hex.len(), 64, "32-byte ed25519 secret key");
        assert!(!a.address.is_empty());
    }

    #[test]
    fn aptos_key_is_deterministic_and_well_formed() {
        let a = derive_aptos(TEST_MNEMONIC, "").unwrap();
        let b = derive_aptos(TEST_MNEMONIC, "").unwrap();
        assert_eq!(a.private_key_hex, b.private_key_hex);
        assert_eq!(a.address, b.address);
        assert_eq!(a.private_key_hex.len(), 64, "32-byte ed25519 secret key");
        assert!(a.address.starts_with("0x"));
        assert_eq!(a.address.len(), 66, "0x + 32-byte sha3-256 digest");
    }

    #[test]
    fn nostr_key_is_deterministic_and_well_formed() {
        let a = derive_nostr(TEST_MNEMONIC, "", 0).unwrap();
        let b = derive_nostr(TEST_MNEMONIC, "", 0).unwrap();
        assert_eq!(a.private_key_hex, b.private_key_hex);
        assert_eq!(a.address, b.address);
        assert_eq!(a.private_key_hex.len(), 64, "32-byte ed25519 secret key");
        assert!(a.address.starts_with("npub"));

        // A different account index must derive a different key -- confirms
        // the account segment of the NIP-06 path is actually load-bearing.
        let c = derive_nostr(TEST_MNEMONIC, "", 1).unwrap();
        assert_ne!(a.private_key_hex, c.private_key_hex);
    }

    #[test]
    fn libp2p_key_is_deterministic_and_well_formed() {
        let k_root = b"test_k_root_32_bytes_exactly_here";
        let (priv1, peer1) = derive_libp2p_key(k_root);
        let (priv2, peer2) = derive_libp2p_key(k_root);
        // Same k_root → same result every time (re-derivation guarantee)
        assert_eq!(priv1, priv2, "private key must be deterministic");
        assert_eq!(peer1, peer2, "peer_id must be deterministic");
        assert_eq!(priv1.len(), 64, "32-byte private key = 64 hex chars");
        assert_eq!(peer1.len(), 64, "32-byte Ed25519 pubkey = 64 hex chars");
        // Different k_root → different key
        let k_root2 = b"different_k_root_32_bytes_here__";
        let (_, peer_other) = derive_libp2p_key(k_root2);
        assert_ne!(
            peer1, peer_other,
            "different k_root must produce different peer_id"
        );
    }

    #[test]
    fn email_local_is_deterministic_and_well_formed() {
        let k_root = b"test_k_root_32_bytes_exactly_here";
        let local1 = derive_email_local(k_root);
        let local2 = derive_email_local(k_root);
        assert_eq!(local1, local2, "email local must be deterministic");
        assert_eq!(local1.len(), 16, "must be exactly 16 hex chars");
        // Different k_root → different local
        let k_root2 = b"different_k_root_32_bytes_here__";
        let local_other = derive_email_local(k_root2);
        assert_ne!(
            local1, local_other,
            "different k_root must produce different email local"
        );
    }

    #[test]
    fn libp2p_key_distinct_from_nostr_key() {
        // The libp2p key must not collide with the Nostr key derived from
        // the same mnemonic — they use different derivation paths.
        const M: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let nostr = derive_nostr(M, "", 0).unwrap();
        // For libp2p we use a synthetic k_root (in birth flow k_root comes from
        // the mnemonic seed; here we just verify non-collision with nostr privkey)
        let seed = b"test_k_root_32_bytes_exactly_here";
        let (libp2p_priv, _) = derive_libp2p_key(seed);
        // They originate from completely different inputs so must be different
        assert_ne!(nostr.private_key_hex, libp2p_priv);
    }

    #[test]
    fn all_chain_keys_from_the_same_mnemonic_are_pairwise_distinct() {
        // Same root mnemonic, six different chains -- every derived
        // private key must be unique. A collision here would mean two
        // chains accidentally share a derivation path.
        let sui = crate::identity::wallet::Wallet::derive_from_mnemonic(TEST_MNEMONIC, "").unwrap();
        let sui_hex = hex::encode(sui.to_bytes());
        let eth = derive_ethereum(TEST_MNEMONIC, "").unwrap().private_key_hex;
        let btc = derive_bitcoin(TEST_MNEMONIC, "").unwrap().private_key_hex;
        let cosmos = derive_cosmos(TEST_MNEMONIC, "").unwrap().private_key_hex;
        let sol = derive_solana(TEST_MNEMONIC, "").unwrap().private_key_hex;
        let aptos = derive_aptos(TEST_MNEMONIC, "").unwrap().private_key_hex;
        let nostr = derive_nostr(TEST_MNEMONIC, "", 0).unwrap().private_key_hex;

        let keys = [sui_hex, eth, btc, cosmos, sol, aptos, nostr];
        for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
                assert_ne!(
                    keys[i], keys[j],
                    "chain {i} and chain {j} must not share a key"
                );
            }
        }
    }
}
