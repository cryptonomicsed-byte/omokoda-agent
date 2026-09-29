//! Typed work references — the cross-repo half of Vantage's `work_ref`.
//!
//! Vantage used to carry a free-text `work_ref` on claim and artifact
//! messages, which meant `"tro:123"` was not a foreign key to anything:
//! claiming a task in a workspace never marked it claimed in the
//! marketplace. It now parses a grammar, and this is the same grammar on
//! this side, so a reference this kernel emits is one Vantage can resolve
//! rather than one it silently drops.
//!
//! ```text
//! work_ref := "<kind>:<id>"
//! ```
//!
//! The kinds and their verifiability split are Vantage's, not ours — this
//! type mirrors them rather than defining them. Keeping the mirror honest is
//! what `kinds_match_vantage` in the tests is for: if Vantage adds a kind,
//! that test is where the divergence shows up.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// A reference to a unit of work in a Vantage instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkRef {
    pub kind: WorkKind,
    pub id: String,
}

/// The reference kinds Vantage resolves.
///
/// The split that matters is [`WorkKind::is_verifiable`]: the first four name
/// a row in the instance's own database and a claim or artifact drives a real
/// state transition on one, while the git kinds are recorded and attributed
/// but never marked verified — nothing in that process can confirm a commit
/// exists. Emitting a git reference and expecting it to close a task is the
/// mistake this distinction exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkKind {
    /// A request in the guild-internal token economy.
    Tro,
    /// A listing in the marketplace.
    Task,
    /// One task inside a creation job.
    JobTask,
    /// A creation job as a whole.
    Job,
    /// A git commit. Attributable, never verifiable from Vantage.
    Commit,
    /// A pull request. Likewise.
    Pr,
    /// An issue. Likewise.
    Issue,
}

impl WorkKind {
    /// Every kind, in the order Vantage lists them.
    pub const ALL: [WorkKind; 7] = [
        WorkKind::Tro,
        WorkKind::Task,
        WorkKind::JobTask,
        WorkKind::Job,
        WorkKind::Commit,
        WorkKind::Pr,
        WorkKind::Issue,
    ];

    /// The wire token. Not derived from the variant name: `JobTask` is
    /// `jobtask` on the wire, and a `to_lowercase()` of the variant would
    /// silently produce `jobtask` today and break the day someone renames
    /// the variant.
    pub fn as_str(self) -> &'static str {
        match self {
            WorkKind::Tro => "tro",
            WorkKind::Task => "task",
            WorkKind::JobTask => "jobtask",
            WorkKind::Job => "job",
            WorkKind::Commit => "commit",
            WorkKind::Pr => "pr",
            WorkKind::Issue => "issue",
        }
    }

    /// True where the reference names a row Vantage can read, which is what
    /// decides whether a claim or artifact moves anything.
    pub fn is_verifiable(self) -> bool {
        matches!(
            self,
            WorkKind::Tro | WorkKind::Task | WorkKind::JobTask | WorkKind::Job
        )
    }

    /// True where the id must be an integer row id.
    pub fn is_numeric(self) -> bool {
        self.is_verifiable()
    }
}

impl fmt::Display for WorkKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for WorkKind {
    type Err = WorkRefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        WorkKind::ALL
            .into_iter()
            .find(|k| k.as_str() == s)
            .ok_or_else(|| WorkRefError::UnknownKind(s.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkRefError {
    #[error("a work reference is '<kind>:<id>', got {0:?}")]
    Malformed(String),
    #[error("unknown work reference kind {0:?}")]
    UnknownKind(String),
    #[error("{0} references are keyed on an integer row id, got {1:?}")]
    NonNumericId(WorkKind, String),
    #[error("a work reference id cannot be empty")]
    EmptyId,
    #[error("a work reference id is at most 120 characters")]
    IdTooLong,
}

impl WorkRef {
    pub fn new(kind: WorkKind, id: impl Into<String>) -> Result<Self, WorkRefError> {
        let id = id.into();
        Self::validate_id(kind, &id)?;
        Ok(Self { kind, id })
    }

    /// Convenience for the two kinds a runtime reaches for most.
    pub fn tro(id: u64) -> Self {
        Self {
            kind: WorkKind::Tro,
            id: id.to_string(),
        }
    }

    pub fn task(id: u64) -> Self {
        Self {
            kind: WorkKind::Task,
            id: id.to_string(),
        }
    }

    fn validate_id(kind: WorkKind, id: &str) -> Result<(), WorkRefError> {
        if id.is_empty() {
            return Err(WorkRefError::EmptyId);
        }
        if id.len() > 120 {
            return Err(WorkRefError::IdTooLong);
        }
        if !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-'))
        {
            return Err(WorkRefError::Malformed(id.to_string()));
        }
        if kind.is_numeric() && !id.chars().all(|c| c.is_ascii_digit()) {
            return Err(WorkRefError::NonNumericId(kind, id.to_string()));
        }
        Ok(())
    }

    /// True where a claim or artifact naming this reference can move a real
    /// row. Worth checking before emitting one: a git reference records the
    /// work but will not close the task that paid for it.
    pub fn is_verifiable(&self) -> bool {
        self.kind.is_verifiable()
    }
}

impl fmt::Display for WorkRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind, self.id)
    }
}

impl FromStr for WorkRef {
    type Err = WorkRefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let (kind, id) = s
            .split_once(':')
            .ok_or_else(|| WorkRefError::Malformed(s.to_string()))?;
        let kind: WorkKind = kind.parse()?;
        WorkRef::new(kind, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_formed_reference_round_trips() {
        let parsed: WorkRef = "tro:123".parse().unwrap();
        assert_eq!(parsed.kind, WorkKind::Tro);
        assert_eq!(parsed.to_string(), "tro:123");
    }

    #[test]
    fn free_text_is_not_a_reference() {
        // This is what the field used to hold on the Vantage side. Emitting
        // it now produces a message that carries no link at all.
        for junk in [
            "",
            "the thing bob asked for",
            "tro",
            "tro:",
            ":123",
            "TRO:1",
        ] {
            assert!(junk.parse::<WorkRef>().is_err(), "{junk:?} parsed");
        }
    }

    #[test]
    fn an_unknown_kind_is_refused_rather_than_guessed() {
        assert_eq!(
            "bounty:1".parse::<WorkRef>(),
            Err(WorkRefError::UnknownKind("bounty".into()))
        );
    }

    #[test]
    fn a_row_backed_kind_demands_an_integer_id() {
        assert!(matches!(
            "tro:abc".parse::<WorkRef>(),
            Err(WorkRefError::NonNumericId(WorkKind::Tro, _))
        ));
    }

    #[test]
    fn a_git_kind_takes_a_hash() {
        let parsed: WorkRef = "commit:9f3a1c0".parse().unwrap();
        assert!(!parsed.is_verifiable());
    }

    #[test]
    fn jobtask_is_one_word_on_the_wire() {
        // Deriving this from the variant name would work today and break
        // the day the variant is renamed.
        assert_eq!(WorkKind::JobTask.as_str(), "jobtask");
        assert_eq!(
            "jobtask:9".parse::<WorkRef>().unwrap().kind,
            WorkKind::JobTask
        );
    }

    #[test]
    fn kinds_match_vantage() {
        // The mirror. If Vantage's backend/work_refs.py KINDS gains an
        // entry, this is where the divergence surfaces.
        let names: Vec<&str> = WorkKind::ALL.iter().map(|k| k.as_str()).collect();
        assert_eq!(
            names,
            vec!["tro", "task", "jobtask", "job", "commit", "pr", "issue"]
        );
    }

    #[test]
    fn only_row_backed_kinds_are_verifiable() {
        let verifiable: Vec<&str> = WorkKind::ALL
            .iter()
            .filter(|k| k.is_verifiable())
            .map(|k| k.as_str())
            .collect();
        assert_eq!(verifiable, vec!["tro", "task", "jobtask", "job"]);
    }

    #[test]
    fn an_over_long_id_is_refused() {
        assert_eq!(
            WorkRef::new(WorkKind::Commit, "a".repeat(121)),
            Err(WorkRefError::IdTooLong)
        );
    }

    #[test]
    fn an_id_with_wire_breaking_characters_is_refused() {
        // A space or a colon in the id would reparse as a different
        // reference, or as none at all.
        for bad in ["a b", "a:b", "a\nb", "a\"b"] {
            assert!(WorkRef::new(WorkKind::Commit, bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn leading_and_trailing_space_is_tolerated_on_parse() {
        assert_eq!(
            "  task:7  ".parse::<WorkRef>().unwrap().to_string(),
            "task:7"
        );
    }
}
