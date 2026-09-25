/// OSO Service Fabric Abstractions — Phase 16 + 17.2
///
/// Three orthogonal interfaces for storage, access control, and compute,
/// plus an `OsoRouter` that selects backends by policy.
/// Phase 17.2 adds `state_commitment` for Freenet → L1 bridge.
pub mod oso_compute;
pub mod oso_router;
pub mod oso_seal;
pub mod oso_storage;
pub mod state_commitment;

pub use oso_compute::{
    AttestationKind, ComputeError, OsoAttestation, OsoComputeJob, OsoComputeProvider,
    OsoComputeResult, OsoJobStatus, OsoWorkload, UcxBackedProvider, VeilSimProvider,
};
pub use oso_router::{OsoRouter, RoutingPolicy};
pub use oso_seal::{
    AccessGrant, AccessPolicy, AccessProvider, EncryptedBlob, GrantId, LocalSealProvider,
    Nip46Provider, SealBackend, SealError, SuiSealProvider,
};
pub use oso_storage::{
    ArweaveProvider, FreenetProvider, LocalFsProvider, StorageBackend, StorageCommitment,
    StorageError, StorageProvider, WalrusProvider,
};
pub use state_commitment::{
    build_commitment, is_significant_transition, submit_to_abci, CommitmentReason,
    StateCommitmentTx,
};
