//! LARQL-over-memory — a small, bounded query language for reading an
//! agent's own `OduDirectory` (see memdir.rs).
//!
//! Distinct from `ifascript::larql`, which is a separate v0.1 engine that
//! queries the *static* 512-entry Òdù divination corpus (not wired into
//! this crate). This module borrows the same honest, grounded-scope
//! discipline -- every clause resolves against a real `OduDirectory`
//! method that already exists (`recall_entity`, `entries_at_path`,
//! `known_entities`, `duplicate_clusters`), no invented fields -- but
//! targets the agent's own conversational memory instead of the corpus.
//!
//! Grammar (case-insensitive keywords):
//!   VERIFY WHERE entity = "Name"
//!   VERIFY WHERE path CONTAINS "substring"
//!   DESCRIBE entities
//!   DESCRIBE duplicates
//!   DESCRIBE reflections
//!   TRACE <node_id>

use crate::memory::dag::CausalMemoryDag;
use crate::memory::memdir::OduDirectory;
use crate::memory::reflection::ReflectionLedger;

#[derive(Debug, Clone, PartialEq)]
pub enum MemoryQuery {
    VerifyEntity(String),
    VerifyPathContains(String),
    DescribeEntities,
    DescribeDuplicates,
    DescribeReflections,
    TraceCausal(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryAnswer {
    pub summary: Vec<String>,
    /// `Some(passed)` for VERIFY; `None` for DESCRIBE.
    pub passed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MemoryQueryError {
    Unrecognized(String),
    MissingQuotedValue,
    MissingNodeId,
}

impl std::fmt::Display for MemoryQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingNodeId => write!(f, "TRACE requires a node id, e.g. TRACE think:123:4"),
            Self::Unrecognized(s) => write!(f, "unrecognized memory query: {s}"),
            Self::MissingQuotedValue => write!(f, "expected a \"quoted value\""),
        }
    }
}

/// Extract the first "quoted" substring, if any.
fn quoted_value(s: &str) -> Option<String> {
    let start = s.find('"')? + 1;
    let end = start + s[start..].find('"')?;
    Some(s[start..end].to_string())
}

pub fn parse_query(input: &str) -> Result<MemoryQuery, MemoryQueryError> {
    let trimmed = input.trim();
    let upper = trimmed.to_ascii_uppercase();

    if upper.starts_with("DESCRIBE") {
        let rest = trimmed[8..].trim().to_ascii_lowercase();
        return match rest.as_str() {
            "entities" => Ok(MemoryQuery::DescribeEntities),
            "duplicates" => Ok(MemoryQuery::DescribeDuplicates),
            "reflections" => Ok(MemoryQuery::DescribeReflections),
            _ => Err(MemoryQueryError::Unrecognized(input.to_string())),
        };
    }

    if upper.starts_with("TRACE") {
        let node_id = trimmed[5..].trim();
        if node_id.is_empty() {
            return Err(MemoryQueryError::MissingNodeId);
        }
        return Ok(MemoryQuery::TraceCausal(node_id.to_string()));
    }

    if upper.starts_with("VERIFY") {
        let rest = &trimmed[6..];
        let rest_upper = rest.to_ascii_uppercase();
        if rest_upper.contains("ENTITY") {
            let value = quoted_value(rest).ok_or(MemoryQueryError::MissingQuotedValue)?;
            return Ok(MemoryQuery::VerifyEntity(value));
        }
        if rest_upper.contains("PATH") && rest_upper.contains("CONTAINS") {
            let value = quoted_value(rest).ok_or(MemoryQueryError::MissingQuotedValue)?;
            return Ok(MemoryQuery::VerifyPathContains(value));
        }
        return Err(MemoryQueryError::Unrecognized(input.to_string()));
    }

    Err(MemoryQueryError::Unrecognized(input.to_string()))
}

pub fn execute(
    query: &MemoryQuery,
    dir: &OduDirectory,
    causal_dag: &CausalMemoryDag,
    reflection: &ReflectionLedger,
) -> MemoryAnswer {
    match query {
        MemoryQuery::VerifyEntity(entity) => {
            let hits = dir.recall_entity(entity);
            let passed = !hits.is_empty();
            let mut summary = vec![if passed {
                format!(
                    "✓ entity \"{entity}\" verified -- {} matching entr{}",
                    hits.len(),
                    if hits.len() == 1 { "y" } else { "ies" }
                )
            } else {
                format!("✗ entity \"{entity}\" not found in memory")
            }];
            summary.extend(hits.iter().take(3).map(|e| {
                format!(
                    "  e.g. [{}] {}",
                    e.path,
                    e.content.chars().take(80).collect::<String>()
                )
            }));
            MemoryAnswer {
                summary,
                passed: Some(passed),
            }
        }
        MemoryQuery::VerifyPathContains(needle) => {
            let hits: Vec<_> = dir
                .entries
                .values()
                .filter(|e| e.path.contains(needle.as_str()))
                .collect();
            let passed = !hits.is_empty();
            let mut summary = vec![if passed {
                format!(
                    "✓ path CONTAINS \"{needle}\" verified -- {} matching entr{}",
                    hits.len(),
                    if hits.len() == 1 { "y" } else { "ies" }
                )
            } else {
                format!("✗ no entries with path containing \"{needle}\"")
            }];
            summary.extend(hits.iter().take(3).map(|e| format!("  e.g. {}", e.path)));
            MemoryAnswer {
                summary,
                passed: Some(passed),
            }
        }
        MemoryQuery::DescribeEntities => {
            let mut entities = dir.known_entities();
            entities.sort_unstable();
            MemoryAnswer {
                summary: entities.iter().map(|e| e.to_string()).collect(),
                passed: None,
            }
        }
        MemoryQuery::DescribeDuplicates => {
            let clusters = dir.duplicate_clusters();
            let summary = clusters
                .iter()
                .map(|(_, ids)| {
                    format!(
                        "{} entries share identical content: {}",
                        ids.len(),
                        ids.join(", ")
                    )
                })
                .collect();
            MemoryAnswer {
                summary,
                passed: None,
            }
        }
        MemoryQuery::DescribeReflections => {
            let summary = reflection
                .entries
                .iter()
                .rev()
                .take(10)
                .map(|r| {
                    format!(
                        "[{}] {} — {}{}",
                        r.timestamp,
                        r.primitive,
                        r.content.chars().take(80).collect::<String>(),
                        r.emotion
                            .as_ref()
                            .map(|e| format!(" ({e})"))
                            .unwrap_or_default()
                    )
                })
                .collect();
            MemoryAnswer {
                summary,
                passed: None,
            }
        }
        MemoryQuery::TraceCausal(node_id) => {
            let chain = causal_dag.causal_chain(node_id);
            if chain.is_empty() {
                MemoryAnswer {
                    summary: vec![format!("✗ no causal node '{node_id}'")],
                    passed: Some(false),
                }
            } else {
                let summary = chain
                    .iter()
                    .enumerate()
                    .map(|(i, n)| {
                        format!(
                            "{}{} [{}] {}",
                            "  ".repeat(i),
                            if i == 0 { "" } else { "← " },
                            n.id,
                            n.content.chars().take(80).collect::<String>()
                        )
                    })
                    .collect();
                MemoryAnswer {
                    summary,
                    passed: Some(true),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::memdir::OduEntry;

    fn dir_with(entries: &[(&str, &str, &str)]) -> OduDirectory {
        let mut dir = OduDirectory::new();
        for (id, content, path) in entries {
            dir.insert(OduEntry::new(*id, *content, *path));
        }
        dir
    }

    #[test]
    fn verify_entity_passes_when_present() {
        let dir = dir_with(&[("e1", "we discussed Vantage today", "think/simplequery")]);
        let q = parse_query(r#"VERIFY WHERE entity = "Vantage""#).unwrap();
        let answer = execute(
            &q,
            &dir,
            &CausalMemoryDag::default(),
            &ReflectionLedger::default(),
        );
        assert_eq!(answer.passed, Some(true));
    }

    #[test]
    fn verify_entity_fails_when_absent() {
        let dir = dir_with(&[("e1", "we discussed lunch today", "think/simplequery")]);
        let q = parse_query(r#"VERIFY WHERE entity = "Vantage""#).unwrap();
        let answer = execute(
            &q,
            &dir,
            &CausalMemoryDag::default(),
            &ReflectionLedger::default(),
        );
        assert_eq!(answer.passed, Some(false));
    }

    #[test]
    fn verify_path_contains() {
        let dir = dir_with(&[("e1", "hello", "think/complextask")]);
        let q = parse_query(r#"VERIFY WHERE path CONTAINS "complextask""#).unwrap();
        let answer = execute(
            &q,
            &dir,
            &CausalMemoryDag::default(),
            &ReflectionLedger::default(),
        );
        assert_eq!(answer.passed, Some(true));
    }

    #[test]
    fn describe_entities_lists_known_entities() {
        let dir = dir_with(&[("e1", "we discussed Vantage and Zangbeto", "think/x")]);
        let q = parse_query("DESCRIBE entities").unwrap();
        let answer = execute(
            &q,
            &dir,
            &CausalMemoryDag::default(),
            &ReflectionLedger::default(),
        );
        // known_entities returns the lowercased index key, not display casing.
        assert!(answer.summary.iter().any(|s| s == "vantage"));
        assert!(answer.summary.iter().any(|s| s == "zangbeto"));
    }

    #[test]
    fn describe_duplicates_finds_exact_content_matches() {
        let mut dir = OduDirectory::new();
        dir.insert(OduEntry::new("a", "identical content here", "p1"));
        dir.insert(OduEntry::new("b", "identical content here", "p2"));
        let q = parse_query("DESCRIBE duplicates").unwrap();
        let answer = execute(
            &q,
            &dir,
            &CausalMemoryDag::default(),
            &ReflectionLedger::default(),
        );
        assert_eq!(answer.summary.len(), 1);
    }

    #[test]
    fn unrecognized_query_is_an_error() {
        assert!(parse_query("WALK something").is_err());
    }

    #[test]
    fn trace_walks_the_causal_chain() {
        let dir = OduDirectory::new();
        let mut dag = CausalMemoryDag::default();
        dag.insert(crate::memory::dag::MemNode::new(
            "root".into(),
            "first thought".into(),
            0,
        ));
        dag.insert(
            crate::memory::dag::MemNode::new("child".into(), "consequence".into(), 1)
                .with_parents(vec!["root".into()]),
        );

        let q = parse_query("TRACE child").unwrap();
        let answer = execute(&q, &dir, &dag, &ReflectionLedger::default());
        assert_eq!(answer.passed, Some(true));
        assert_eq!(answer.summary.len(), 2);
        assert!(answer.summary[0].contains("consequence"));
        assert!(answer.summary[1].contains("first thought"));
    }

    #[test]
    fn trace_missing_node_fails_cleanly() {
        let dir = OduDirectory::new();
        let q = parse_query("TRACE nonexistent").unwrap();
        let answer = execute(
            &q,
            &dir,
            &CausalMemoryDag::default(),
            &ReflectionLedger::default(),
        );
        assert_eq!(answer.passed, Some(false));
    }

    #[test]
    fn trace_without_node_id_is_an_error() {
        assert_eq!(parse_query("TRACE"), Err(MemoryQueryError::MissingNodeId));
        assert_eq!(
            parse_query("TRACE   "),
            Err(MemoryQueryError::MissingNodeId)
        );
    }

    #[test]
    fn describe_reflections_shows_recent_entries_with_emotion() {
        let dir = OduDirectory::new();
        let mut ledger = ReflectionLedger::default();
        let emotion = crate::emotion::EmotionState::birth();
        ledger.record_with_emotion("think", "hello there", 100, &emotion);
        ledger.record("act", "did a thing", 200);

        let q = parse_query("DESCRIBE reflections").unwrap();
        let answer = execute(&q, &dir, &CausalMemoryDag::default(), &ledger);
        assert_eq!(answer.summary.len(), 2);
        // Most recent first.
        assert!(answer.summary[0].contains("did a thing"));
        assert!(answer.summary[1].contains("hello there"));
        assert!(answer.summary[1].contains("energy="));
    }

    #[test]
    fn missing_quoted_value_is_an_error() {
        assert!(matches!(
            parse_query("VERIFY WHERE entity = novalue"),
            Err(MemoryQueryError::MissingQuotedValue)
        ));
    }
}
