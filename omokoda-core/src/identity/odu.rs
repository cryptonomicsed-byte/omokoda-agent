use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OduSeed(pub [u8; 32]);

impl OduSeed {
    pub fn new(seed: [u8; 32]) -> Self {
        Self(seed)
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn len(&self) -> usize {
        32
    }

    pub fn is_empty(&self) -> bool {
        false
    }
}

impl AsRef<[u8]> for OduSeed {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OduIdentity {
    pub primary_index: u8,
    pub mnemonic: String,
}

/// An agent's identity index resolved into the IfáScript Odù corpus: the
/// canonical name, action vessel, and primary prescription that the index
/// `primary_index` stands for. This is the bridge between BIPỌ̀N39 identity
/// (the XOR-reduced index) and the meaning IfáScript assigns it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OduSign {
    pub index: u8,
    pub name: String,
    pub vessel: String,
    pub prescription: Option<String>,
}

impl OduIdentity {
    /// Resolve this identity's `primary_index` into its IfáScript Odù sign.
    ///
    /// `primary_index` is produced by BIPỌ̀N39 (an XOR reduction of the
    /// mnemonic's token indices over the 256 base Odù). On its own it is just a
    /// number; `sign()` looks it up in the IfáScript corpus so the identity
    /// carries its actual Odù — name, vessel, and prescription — not a bare u8.
    pub fn sign(&self) -> OduSign {
        let odu = ifascript::get_odu(self.primary_index);
        OduSign {
            index: self.primary_index,
            name: odu.universal_name.to_string(),
            vessel: format!("{:?}", odu.vessel),
            prescription: odu.prescriptions.first().map(|p| p.to_string()),
        }
    }

    /// Derives the Odù `primary_index` that WOULD be assigned to an agent born
    /// from `entropy` — i.e., it follows the canonical Bipon39 path:
    ///   entropy → mnemonic → indices → XOR-reduced index.
    ///
    /// This makes the entropy→Odù derivation an explicit, auditable one-liner
    /// without requiring the caller to hold a mnemonic.  Used during birth
    /// validation and ancestry/lineage derivation.
    pub fn primary_index_for_entropy(entropy: &[u8]) -> u8 {
        use crate::identity::bipon39::Bipon39;
        let mnemonic = Bipon39::entropy_to_mnemonic(entropy);
        let indices = Bipon39::mnemonic_to_indices(&mnemonic).unwrap_or_default();
        Bipon39::get_odu_index(&indices)
    }

    /// Returns `true` when this `OduIdentity`'s `primary_index` is consistent
    /// with the given raw birth entropy bytes — i.e., deriving the index via
    /// the Bipon39 path produces the same value that is stored here.
    ///
    /// Intended for birth receipt verification: after loading a persisted
    /// `OduIdentity`, pass the original entropy to confirm the index was not
    /// tampered with.
    pub fn birth_entropy_matches(&self, entropy: &[u8]) -> bool {
        Self::primary_index_for_entropy(entropy) == self.primary_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_resolves_index_into_ifascript_odu() {
        let identity = OduIdentity {
            primary_index: 7,
            mnemonic: "Ogbe leads the way".to_string(),
        };
        let sign = identity.sign();
        // The sign preserves the identity index and pulls a real name/vessel
        // from the IfáScript corpus (never an empty string).
        assert_eq!(sign.index, 7);
        assert!(!sign.name.is_empty());
        assert!(!sign.vessel.is_empty());
    }

    #[test]
    fn primary_index_for_entropy_is_deterministic() {
        let entropy = b"test-entropy-bytes-for-odu-derivation";
        let index_a = OduIdentity::primary_index_for_entropy(entropy);
        let index_b = OduIdentity::primary_index_for_entropy(entropy);
        assert_eq!(index_a, index_b, "same entropy must always produce the same primary_index");
    }

    #[test]
    fn birth_entropy_matches_round_trips() {
        use crate::identity::bipon39::Bipon39;
        let entropy = b"round-trip-entropy-for-birth-validation";
        let mnemonic = Bipon39::entropy_to_mnemonic(entropy);
        let indices = Bipon39::mnemonic_to_indices(&mnemonic).unwrap_or_default();
        let primary_index = Bipon39::get_odu_index(&indices);
        let identity = OduIdentity { primary_index, mnemonic };
        assert!(
            identity.birth_entropy_matches(entropy),
            "OduIdentity derived from entropy must pass birth_entropy_matches for that same entropy"
        );
    }
}
