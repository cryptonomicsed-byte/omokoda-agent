use crate::emotion::EmotionState;
use crate::lifecycle::sensor::SensorReading;
use serde::{Deserialize, Serialize};

/// Summary passed to `sleep()` for consolidation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub message_count: u64,
    pub key_topics: Vec<String>,
    pub notable_events: Vec<String>,
}

/// Manages the three lifecycle phases of a sovereign agent session.
pub struct SomaLifecycle {
    pub emotion: EmotionState,
    pub session_start: u64,
    pub message_count: u64,
}

impl SomaLifecycle {
    /// Wake up: read sensors, initialize EmotionState from device reality.
    pub fn wake_up() -> Self {
        let sensors = SensorReading::read();
        let emotion = EmotionState::birth().update_from_sensors(
            sensors.battery_pct,
            sensors.temp_c,
            sensors.hour,
        );
        let session_start = now_secs();
        log::info!(
            "soma::wake_up battery={}% temp={}°C hour={} energy={:.2} tension={:.2}",
            sensors.battery_pct,
            sensors.temp_c,
            sensors.hour,
            emotion.energy,
            emotion.tension,
        );
        Self {
            emotion,
            session_start,
            message_count: 0,
        }
    }

    /// Pulse: called per message. Updates emotion from prompt content.
    /// Returns true if the exchange was significant enough to store as episodic memory.
    pub fn pulse(&mut self, message: &str, _role: &str) -> bool {
        self.emotion = self.emotion.after_think(message);
        self.message_count += 1;
        // Significant = distress or high vitality change
        let significant = self.emotion.is_tense() || self.emotion.is_connected();
        significant
    }

    /// Sleep: consolidate session learning, reset for next session.
    pub fn sleep(&self, summary: &SessionSummary) {
        let uptime = now_secs().saturating_sub(self.session_start);
        log::info!(
            "soma::sleep uptime={}s messages={} topics={:?} vitality={:.2}",
            uptime,
            summary.message_count,
            summary.key_topics,
            self.emotion.vitality(),
        );
        // In the full system: write LPM update, persist episodic memories.
        // Here we emit a structured log that minipae can pick up.
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wake_up_produces_valid_emotion() {
        let soma = SomaLifecycle::wake_up();
        assert!((0.0..=1.0).contains(&soma.emotion.energy));
        assert!((0.0..=1.0).contains(&soma.emotion.tension));
    }

    #[test]
    fn pulse_increments_message_count() {
        let mut soma = SomaLifecycle::wake_up();
        soma.pulse("hello there", "user");
        assert_eq!(soma.message_count, 1);
    }

    #[test]
    fn sleep_does_not_panic() {
        let soma = SomaLifecycle::wake_up();
        let summary = SessionSummary {
            message_count: 3,
            key_topics: vec!["rust".to_string()],
            notable_events: vec![],
        };
        soma.sleep(&summary);
    }
}
