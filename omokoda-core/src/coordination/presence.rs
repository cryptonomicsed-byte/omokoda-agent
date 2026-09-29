//! What this agent is doing, in the vocabulary Vantage's Conductor accepts.
//!
//! Socket liveness answers "is it there". This answers "is it worth waiting
//! for", which is the question a room full of agents actually needs answered
//! before it hands anyone the next unit of work.
//!
//! The vocabulary is closed on both sides, and closed on purpose: a scheduler
//! cannot act on "thinking hard about the parser", and once arbitrary strings
//! are accepted they can never be withdrawn. Sending a state outside it is an
//! error here rather than at the far end, so it surfaces at the call site.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkState {
    /// Idle and ready for work.
    Available,
    /// Reasoning, not yet acting. Still worth handing work to.
    Thinking,
    /// Executing something. Present, but busy.
    Working,
    /// Waiting on something outside this agent.
    Blocked,
    /// Finished, and waiting on a human or a peer to look.
    NeedsReview,
    /// Not participating.
    Offline,
}

impl WorkState {
    pub const ALL: [WorkState; 6] = [
        WorkState::Available,
        WorkState::Thinking,
        WorkState::Working,
        WorkState::Blocked,
        WorkState::NeedsReview,
        WorkState::Offline,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            WorkState::Available => "available",
            WorkState::Thinking => "thinking",
            WorkState::Working => "working",
            WorkState::Blocked => "blocked",
            WorkState::NeedsReview => "needs_review",
            WorkState::Offline => "offline",
        }
    }

    /// Whether a scheduler should route new work here.
    ///
    /// `Blocked` and `NeedsReview` both mean somebody else has to move
    /// first; handing more work to an agent in either is how a queue
    /// silently stalls.
    pub fn is_routable(self) -> bool {
        matches!(self, WorkState::Available | WorkState::Thinking)
    }
}

impl fmt::Display for WorkState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for WorkState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        WorkState::ALL
            .into_iter()
            .find(|w| w.as_str() == s)
            .ok_or_else(|| format!("unknown work state {s:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vocabulary_matches_the_conductors() {
        // Conductor.Flow's @work_states and Vantage's presence.STATES. Three
        // copies of one closed vocabulary drift silently; this is the copy
        // that makes the drift loud on this side.
        let names: Vec<&str> = WorkState::ALL.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "available",
                "thinking",
                "working",
                "blocked",
                "needs_review",
                "offline"
            ]
        );
    }

    #[test]
    fn blocked_and_needs_review_are_not_routable() {
        assert!(!WorkState::Blocked.is_routable());
        assert!(!WorkState::NeedsReview.is_routable());
        assert!(!WorkState::Offline.is_routable());
    }

    #[test]
    fn thinking_still_counts_as_available_for_work() {
        assert!(WorkState::Thinking.is_routable());
        assert!(WorkState::Available.is_routable());
    }

    #[test]
    fn an_unknown_state_is_refused_at_the_call_site() {
        assert!("vibing".parse::<WorkState>().is_err());
        assert_eq!(
            "needs_review".parse::<WorkState>().unwrap(),
            WorkState::NeedsReview
        );
    }

    #[test]
    fn serialisation_uses_the_wire_spelling() {
        // needs_review, not needsReview or NeedsReview.
        let json = serde_json::to_string(&WorkState::NeedsReview).unwrap();
        assert_eq!(json, "\"needs_review\"");
    }
}
