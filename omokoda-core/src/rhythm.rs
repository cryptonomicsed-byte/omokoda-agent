use chrono::{Datelike, Timelike, Utc, Weekday};
use serde_json::Value;

// ─── Kóòdù Daily Resonance ────────────────────────────────────────────────────

const KOODU_SUNDAY: &str = include_str!("koodu/sunday.json");
const KOODU_MONDAY: &str = include_str!("koodu/monday.json");
const KOODU_TUESDAY: &str = include_str!("koodu/tuesday.json");
const KOODU_WEDNESDAY: &str = include_str!("koodu/wednesday.json");
const KOODU_THURSDAY: &str = include_str!("koodu/thursday.json");
const KOODU_FRIDAY: &str = include_str!("koodu/friday.json");
const KOODU_SATURDAY: &str = include_str!("koodu/saturday.json");

/// Raw Kóòdù codex JSON for a given weekday index (0 = Sunday .. 6 =
/// Saturday). The single source of truth for the 7 embedded files, so
/// other modules (e.g. `tools::mesh_tools::daily_resonance`) don't
/// maintain their own duplicate `include_str!` set that can drift out of
/// sync with this one.
pub fn raw_codex_for_weekday(weekday: u8) -> &'static str {
    match weekday % 7 {
        0 => KOODU_SUNDAY,
        1 => KOODU_MONDAY,
        2 => KOODU_TUESDAY,
        3 => KOODU_WEDNESDAY,
        4 => KOODU_THURSDAY,
        5 => KOODU_FRIDAY,
        _ => KOODU_SATURDAY,
    }
}

/// OSOVM_CODEX.md §42 (locked canon, owner 2026-08-22): user-facing surfaces
/// use universal wording only, never the internal Yorùbá/Òrìṣà name. The
/// embedded Kóòdù JSON files are internal canon content and stay as-is on
/// disk, but the two fields that name the day's Òrìṣà outright -- the
/// top-level `archetype` field and the `facets` entry literally labeled
/// `"Òrìṣà"` -- must be translated before this data reaches `GET
/// /v1/rhythm/today` or any other caller. Same weekday ordering as
/// `agent_resonance`'s match arms below (Sunday=Èṣù .. Saturday=Ọbàtálá).
fn universal_archetype_for_weekday(weekday: u8) -> &'static str {
    match weekday % 7 {
        0 => "Access / Identity",   // Sunday -- Èṣù
        1 => "Score / Reputation",  // Monday -- Ṣàngó
        2 => "History / Memory",    // Tuesday -- Ọ̀ṣun
        3 => "Spawn / Create",      // Wednesday -- Yemọja
        4 => "Sync / Flow",         // Thursday -- Ọ̀yá
        5 => "Run / Action",        // Friday -- Ògún
        _ => "Policy / Rules",      // Saturday -- Ọbàtálá
    }
}

/// Rewrites the `archetype` field and the `"Òrìṣà"` facet (if present) in a
/// parsed Kóòdù codex JSON to their §42 universal terms, in place. Leaves
/// every other field (day name, element, tone, etc.) untouched -- those
/// aren't Òrìṣà-name leaks, and a full field-by-field pass over the rest of
/// the bilingual codex schema is a separate, larger piece of work.
fn universalize_archetype(value: &mut Value, weekday: u8) {
    let universal = universal_archetype_for_weekday(weekday);
    if let Some(obj) = value.as_object_mut() {
        if obj.contains_key("archetype") {
            obj.insert("archetype".to_string(), Value::String(universal.to_string()));
        }
        if let Some(facets) = obj.get_mut("facets").and_then(|f| f.as_array_mut()) {
            for facet in facets.iter_mut() {
                if facet.get("name").and_then(|n| n.as_str()) == Some("Òrìṣà") {
                    if let Some(f_obj) = facet.as_object_mut() {
                        f_obj.insert("name".to_string(), Value::String("Archetype".to_string()));
                        f_obj.insert("value".to_string(), Value::String(universal.to_string()));
                    }
                }
            }
        }
        // house_role's field naming the same day's ruling deity (distinct
        // from the top-level `archetype`, which house_role also has -- but
        // house_role.archetype is already an English word, e.g. "Oracle", a
        // character role, not the Òrìṣà name). Caught by the regression
        // test below; the `archetype` and `facets` handling above didn't
        // reach this nested object. The key itself is spelled
        // inconsistently across the 7 source files -- "orisa" in six of
        // them, "orisha" (with an h) in saturday.json only -- a pre-existing
        // data quality issue in the source JSON, not something introduced
        // here; handle both spellings rather than fixing only the one this
        // file happened to be checked against.
        if let Some(house_role) = obj.get_mut("house_role").and_then(|h| h.as_object_mut()) {
            if house_role.contains_key("orisa") {
                house_role.insert("orisa".to_string(), Value::String(universal.to_string()));
            }
            if house_role.contains_key("orisha") {
                house_role.insert("orisha".to_string(), Value::String(universal.to_string()));
            }
        }
    }
}

/// Returns today's Kóòdù resonance JSON parsed as a serde_json::Value --
/// "what day is it for the hive right now," a legitimate wall-clock
/// question, identical for every agent at a given moment. For an
/// individual agent's own permanent resonance (which does NOT change day
/// to day), use `agent_resonance` instead.
pub fn today_resonance() -> Value {
    let weekday_idx = Utc::now().weekday().num_days_from_sunday() as u8;
    let raw = match Utc::now().weekday() {
        Weekday::Sun => KOODU_SUNDAY,
        Weekday::Mon => KOODU_MONDAY,
        Weekday::Tue => KOODU_TUESDAY,
        Weekday::Wed => KOODU_WEDNESDAY,
        Weekday::Thu => KOODU_THURSDAY,
        Weekday::Fri => KOODU_FRIDAY,
        Weekday::Sat => KOODU_SATURDAY,
    };
    let mut parsed: Value =
        serde_json::from_str(raw).unwrap_or(serde_json::json!({"error": "parse failed"}));
    universalize_archetype(&mut parsed, weekday_idx);
    parsed
}

/// An agent's own permanent Kóòdù resonance, keyed on the `day_osa` layer
/// of her Spiral Calendar signature (derived once from her birth
/// timestamp, never from "now" -- see `AgentCore::spiral_time`). Uses the
/// same day-cycle Òrìṣà ordering the Kóòdù JSON files themselves are
/// authored against, so `Macro::Sango` always resolves to monday.json
/// regardless of what day it actually is when this is called.
pub fn agent_resonance(day_osa: bipon39::Macro) -> Value {
    use bipon39::Macro;
    let (raw, weekday_idx) = match day_osa {
        Macro::Esu => (KOODU_SUNDAY, 0),
        Macro::Sango => (KOODU_MONDAY, 1),
        Macro::Osun => (KOODU_TUESDAY, 2),
        Macro::Yemoja => (KOODU_WEDNESDAY, 3),
        Macro::Oya => (KOODU_THURSDAY, 4),
        Macro::Ogun => (KOODU_FRIDAY, 5),
        Macro::Obatala => (KOODU_SATURDAY, 6),
    };
    let mut parsed: Value =
        serde_json::from_str(raw).unwrap_or(serde_json::json!({"error": "parse failed"}));
    universalize_archetype(&mut parsed, weekday_idx);
    parsed
}

/// Irreversible action categories that must pause on Sabbath.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionReversibility {
    Reversible,
    Irreversible,
}

/// Result of a rhythm gate check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RhythmDecision {
    Allow,
    /// Sabbath is active — action is queued, not denied.
    QueuedForSabbathEnd {
        reason: String,
    },
    /// Cooldown is active for this tool.
    Cooldown {
        remaining_secs: u64,
    },
}

pub struct RhythmGate;

impl RhythmGate {
    /// Returns true if it's currently the UTC Sabbath (Saturday).
    pub fn is_sabbath() -> bool {
        Utc::now().weekday() == Weekday::Sat
    }

    /// Returns the current UTC day name.
    pub fn current_day_name() -> &'static str {
        match Utc::now().weekday() {
            Weekday::Sun => "Sunday",
            Weekday::Mon => "Monday",
            Weekday::Tue => "Tuesday",
            Weekday::Wed => "Wednesday",
            Weekday::Thu => "Thursday",
            Weekday::Fri => "Friday",
            Weekday::Sat => "Saturday",
        }
    }

    /// Returns seconds remaining in the current Sabbath (if active), else 0.
    pub fn sabbath_seconds_remaining() -> u64 {
        if !Self::is_sabbath() {
            return 0;
        }
        let now = Utc::now();
        // Sabbath ends at midnight Saturday → Sunday UTC
        let secs_into_day = (now.num_seconds_from_midnight()) as u64;
        86_400u64.saturating_sub(secs_into_day)
    }

    /// Gate an action based on reversibility and current rhythm state.
    pub fn check(
        action: &str,
        reversibility: ActionReversibility,
        cooldown_remaining_secs: u64,
    ) -> RhythmDecision {
        if cooldown_remaining_secs > 0 {
            return RhythmDecision::Cooldown {
                remaining_secs: cooldown_remaining_secs,
            };
        }
        if reversibility == ActionReversibility::Irreversible && Self::is_sabbath() {
            return RhythmDecision::QueuedForSabbathEnd {
                reason: format!(
                    "Action '{}' is irreversible. Sabbath is active (UTC Saturday). \
                     This action will execute when Sabbath ends. \
                     {} seconds remaining.",
                    action,
                    Self::sabbath_seconds_remaining()
                ),
            };
        }
        RhythmDecision::Allow
    }

    /// Classify whether a tool action is irreversible.
    pub fn classify_reversibility(tool: &str) -> ActionReversibility {
        match tool {
            "write_file"
            | "delete_file"
            | "bash"
            | "api_connect"
            | "agent_orchestration"
            | "self_modification"
            // zero patch mutates the agent's own program graph — the purest
            // form of self-modification. Queued on the Sabbath while the
            // dream engine runs its REM cycle.
            | "zero"
            | "multi_agent_fabric" => ActionReversibility::Irreversible,
            _ => ActionReversibility::Reversible,
        }
    }
}

/// Per-agent per-tool cooldown tracker (in-memory).
#[derive(Debug, Clone, Default)]
pub struct CooldownTracker {
    // (tool_name, expiry_unix_timestamp)
    cooldowns: Vec<(String, u64)>,
}

impl CooldownTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a cooldown for `tool` lasting `duration_secs` from now.
    pub fn set(&mut self, tool: &str, duration_secs: u64) {
        let expiry = current_unix_timestamp() + duration_secs;
        if let Some(entry) = self.cooldowns.iter_mut().find(|(t, _)| t == tool) {
            entry.1 = expiry;
        } else {
            self.cooldowns.push((tool.to_string(), expiry));
        }
    }

    /// Returns remaining cooldown seconds for `tool`, or 0 if none.
    pub fn remaining(&self, tool: &str) -> u64 {
        let now = current_unix_timestamp();
        self.cooldowns
            .iter()
            .find(|(t, _)| t == tool)
            .map(|(_, expiry)| expiry.saturating_sub(now))
            .unwrap_or(0)
    }

    /// Returns true if any tool has an active (non-expired) cooldown.
    pub fn has_any_active(&self) -> bool {
        let now = current_unix_timestamp();
        self.cooldowns.iter().any(|(_, expiry)| *expiry > now)
    }

    /// Removes expired cooldowns.
    pub fn prune(&mut self) {
        let now = current_unix_timestamp();
        self.cooldowns.retain(|(_, expiry)| *expiry > now);
    }
}

fn current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_file_is_irreversible() {
        assert_eq!(
            RhythmGate::classify_reversibility("write_file"),
            ActionReversibility::Irreversible
        );
    }

    #[test]
    fn web_search_is_reversible() {
        assert_eq!(
            RhythmGate::classify_reversibility("web_search"),
            ActionReversibility::Reversible
        );
    }

    #[test]
    fn cooldown_remaining_zero_when_none() {
        let tracker = CooldownTracker::new();
        assert_eq!(tracker.remaining("some_tool"), 0);
    }

    #[test]
    fn cooldown_set_and_active() {
        let mut tracker = CooldownTracker::new();
        tracker.set("bash", 60);
        assert!(tracker.remaining("bash") > 0);
        assert!(tracker.remaining("bash") <= 60);
    }

    #[test]
    fn cooldown_triggers_gate() {
        let decision = RhythmGate::check("bash", ActionReversibility::Reversible, 30);
        assert!(matches!(
            decision,
            RhythmDecision::Cooldown { remaining_secs: 30 }
        ));
    }

    #[test]
    fn reversible_action_allowed_any_day() {
        // web_search is reversible — should always be allowed (no cooldown)
        let decision = RhythmGate::check("web_search", ActionReversibility::Reversible, 0);
        assert_eq!(decision, RhythmDecision::Allow);
    }

    #[test]
    fn day_name_is_valid() {
        let day = RhythmGate::current_day_name();
        let valid = [
            "Sunday",
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
        ];
        assert!(valid.contains(&day));
    }

    #[test]
    fn all_seven_koodu_codices_parse_with_49_unique_facets() {
        for weekday in 0..7u8 {
            let raw = raw_codex_for_weekday(weekday);
            let parsed: Value = serde_json::from_str(raw)
                .unwrap_or_else(|e| panic!("weekday {weekday} codex failed to parse: {e}"));
            let facets = parsed["facets"]
                .as_array()
                .unwrap_or_else(|| panic!("weekday {weekday} codex missing 'facets' array"));
            assert_eq!(
                facets.len(),
                49,
                "weekday {weekday} codex has {} facets, expected 49",
                facets.len()
            );
            let mut ids: Vec<u64> = facets
                .iter()
                .map(|f| f["id"].as_u64().expect("facet missing numeric id"))
                .collect();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(
                ids,
                (1..=49).collect::<Vec<u64>>(),
                "weekday {weekday} codex facet ids are not exactly 1..=49 with no gaps/dupes"
            );
        }
    }

    #[test]
    fn facet_names_are_consistent_across_all_seven_days() {
        use std::collections::HashMap;
        let mut names_by_id: HashMap<u64, &str> = HashMap::new();
        let parsed: Vec<Value> = (0..7u8)
            .map(|w| serde_json::from_str(raw_codex_for_weekday(w)).unwrap())
            .collect();
        for day in &parsed {
            for facet in day["facets"].as_array().unwrap() {
                let id = facet["id"].as_u64().unwrap();
                let name = facet["name"].as_str().unwrap();
                match names_by_id.get(&id) {
                    None => {
                        names_by_id.insert(id, name);
                    }
                    Some(expected) => assert_eq!(
                        *expected, name,
                        "facet id {id} has inconsistent names across days"
                    ),
                }
            }
        }
    }

    // OSOVM_CODEX.md §42: regression lock for the "who are you -> Sango"
    // leak class -- scoped to the *structured identification* fields this
    // fix actually targets: the top-level `archetype` field, the facet
    // literally named `"Òrìṣà"`, and `house_role`'s orisa/orisha key.
    //
    // Deliberately NOT a whole-document substring scan: the 49-facet
    // schema's free-text *values* also mention the day's Òrìṣà by name in
    // ordinary descriptive prose in places unrelated to self-identification
    // -- e.g. facet 32 ("Metal") on Tuesday reads "Copper (Ọ̀ṣun's
    // metal)". That's authored flavor-text content across potentially many
    // of the 7×49 facets, not a self-identification leak, and scrubbing it
    // is a much larger content-authoring task than this fix -- same
    // carve-out class as the `yoruba_name` day-naming field and the
    // odu_ifa corpus exception §42 itself calls out as "under review."
    // Also caught here: `today_resonance()`'s equivalent test was
    // day-of-week dependent (only failed if run on a day whose facets
    // happen to mention the name in prose) -- scoping to structured fields
    // makes both tests deterministic regardless of which day they run.
    #[test]
    fn agent_resonance_never_leaks_the_raw_orisha_name_in_structured_fields() {
        use bipon39::Macro;
        for m in [
            Macro::Esu,
            Macro::Sango,
            Macro::Osun,
            Macro::Yemoja,
            Macro::Oya,
            Macro::Ogun,
            Macro::Obatala,
        ] {
            let v = agent_resonance(m);
            assert_eq!(v["archetype"].as_str().unwrap(), universal_archetype_for_weekday_for_macro(m));
            let facets = v["facets"].as_array().unwrap();
            assert!(
                facets.iter().all(|f| f["name"].as_str() != Some("Òrìṣà")),
                "a facet is still literally named \"Òrìṣà\" for {}",
                m.name()
            );
            if let Some(house_role) = v.get("house_role").and_then(|h| h.as_object()) {
                for key in ["orisa", "orisha"] {
                    if let Some(val) = house_role.get(key).and_then(|v| v.as_str()) {
                        assert_eq!(
                            val,
                            universal_archetype_for_weekday_for_macro(m),
                            "house_role.{key} still carries the raw name for {}",
                            m.name()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn today_resonance_never_leaks_the_raw_orisha_name_in_structured_fields() {
        let v = today_resonance();
        assert!(v["archetype"].as_str().is_some());
        let raw_names = ["Ṣàngó", "Ọ̀ṣun", "Yemọja", "Ọ̀yá", "Ògún", "Ọbàtálá", "Èṣù"];
        assert!(
            !raw_names.contains(&v["archetype"].as_str().unwrap()),
            "today_resonance()'s archetype field is still a raw Orisha name: {}",
            v["archetype"]
        );
        let facets = v["facets"].as_array().unwrap();
        assert!(
            facets.iter().all(|f| f["name"].as_str() != Some("Òrìṣà")),
            "a facet is still literally named \"Òrìṣà\""
        );
        if let Some(house_role) = v.get("house_role").and_then(|h| h.as_object()) {
            for key in ["orisa", "orisha"] {
                if let Some(val) = house_role.get(key).and_then(|v| v.as_str()) {
                    assert!(
                        !raw_names.contains(&val),
                        "house_role.{key} is still a raw Orisha name: {val}"
                    );
                }
            }
        }
    }

    /// Test-only helper mirroring the same weekday order agent_resonance
    /// uses, so the assertion above doesn't hardcode index numbers.
    fn universal_archetype_for_weekday_for_macro(m: bipon39::Macro) -> &'static str {
        use bipon39::Macro;
        let idx = match m {
            Macro::Esu => 0,
            Macro::Sango => 1,
            Macro::Osun => 2,
            Macro::Yemoja => 3,
            Macro::Oya => 4,
            Macro::Ogun => 5,
            Macro::Obatala => 6,
        };
        universal_archetype_for_weekday(idx)
    }
}

// ─── Action Cadence / Scheduler Metadata ─────────────────────────────────────
//
// The Calabash corpus declares a Cadence: block per directive.
// These types capture that metadata and drive the ActionTransaction scheduler.

use serde::{Deserialize, Serialize};

/// What event class triggers an action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerKind {
    /// Fires when a specific system event arrives (e.g. "file_saved", "agent_message").
    EventDriven { event: String },
    /// Fires when a runtime state condition becomes true.
    StateDriven { condition: String },
    /// Fires on a cron-like schedule.
    Scheduled { cron: String },
    /// Fires immediately when invoked — no deferral.
    Immediate,
}

/// Cadence descriptor attached to a Calabash directive.
/// Tells the scheduler WHEN to run the action and how to pace it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionCadence {
    /// What triggers execution of this action.
    pub trigger: TriggerKind,
    /// Minimum seconds between two consecutive executions of this action.
    /// None = no enforced cooldown (fire as fast as the trigger allows).
    pub cooldown_secs: Option<u64>,
    /// Maximum number of executions in a sliding window. None = unlimited.
    pub max_per_window: Option<u32>,
    /// Window size in seconds for `max_per_window`. None = unlimited.
    pub window_secs: Option<u64>,
    /// Seconds after proposal within which the action must begin (else: Expired).
    pub deadline_secs: Option<u64>,
}

impl ActionCadence {
    /// Returns true if the action deadline has elapsed given the proposal timestamp.
    pub fn is_expired(&self, proposed_at_secs: u64) -> bool {
        if let Some(deadline) = self.deadline_secs {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            now > proposed_at_secs + deadline
        } else {
            false
        }
    }

    /// Immediate cadence — runs right away, no constraints.
    pub fn immediate() -> Self {
        Self {
            trigger: TriggerKind::Immediate,
            cooldown_secs: None,
            max_per_window: None,
            window_secs: None,
            deadline_secs: None,
        }
    }

    /// Event-driven cadence with a minimum cooldown between executions.
    pub fn on_event(event: impl Into<String>, cooldown_secs: u64) -> Self {
        Self {
            trigger: TriggerKind::EventDriven { event: event.into() },
            cooldown_secs: Some(cooldown_secs),
            max_per_window: None,
            window_secs: None,
            deadline_secs: None,
        }
    }

    /// Scheduled cadence (cron syntax) with a rate limit.
    pub fn scheduled(cron: impl Into<String>, max_per_day: u32) -> Self {
        Self {
            trigger: TriggerKind::Scheduled { cron: cron.into() },
            cooldown_secs: None,
            max_per_window: Some(max_per_day),
            window_secs: Some(86400),
            deadline_secs: None,
        }
    }
}

#[cfg(test)]
mod cadence_tests {
    use super::*;

    #[test]
    fn immediate_cadence_not_expired() {
        let c = ActionCadence::immediate();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(!c.is_expired(now));
    }

    #[test]
    fn deadline_expired_in_past() {
        let mut c = ActionCadence::immediate();
        c.deadline_secs = Some(1); // 1 second deadline
        let old_proposal = 0u64; // proposed at epoch — long past deadline
        assert!(c.is_expired(old_proposal));
    }

    #[test]
    fn on_event_sets_cooldown() {
        let c = ActionCadence::on_event("file_saved", 30);
        assert_eq!(c.cooldown_secs, Some(30));
        assert!(matches!(c.trigger, TriggerKind::EventDriven { .. }));
    }

    #[test]
    fn scheduled_sets_window() {
        let c = ActionCadence::scheduled("0 * * * *", 24);
        assert_eq!(c.max_per_window, Some(24));
        assert_eq!(c.window_secs, Some(86400));
    }
}
