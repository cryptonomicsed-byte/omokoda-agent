use serde::{Deserialize, Serialize};

/// Hardware sensor snapshot from the host device.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SensorReading {
    /// Battery level 0-100
    pub battery_pct: u8,
    /// CPU/device temperature in °C
    pub temp_c: f32,
    /// Hour of day 0-23
    pub hour: u8,
}

impl SensorReading {
    /// Safe defaults when sensor data is unavailable.
    pub fn default_safe() -> Self {
        Self {
            battery_pct: 100,
            temp_c: 30.0,
            hour: 12,
        }
    }

    /// Read live sensor data. On Termux, shells out to termux-battery-status.
    /// Falls back to defaults on non-Termux or parse errors.
    pub fn read() -> Self {
        #[cfg(target_os = "android")]
        {
            if let Some(reading) = read_termux() {
                return reading;
            }
        }
        Self::default_safe()
    }
}

#[cfg(target_os = "android")]
fn read_termux() -> Option<SensorReading> {
    let output = std::process::Command::new("termux-battery-status")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = std::str::from_utf8(&output.stdout).ok()?;
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let battery_pct = v["percentage"].as_f64()? as u8;
    let temp_c = v["temperature"].as_f64().unwrap_or(30.0) as f32;
    let hour = current_hour();
    Some(SensorReading {
        battery_pct,
        temp_c,
        hour,
    })
}

fn current_hour() -> u8 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    ((secs % 86400) / 3600) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_safe_is_valid() {
        let r = SensorReading::default_safe();
        assert_eq!(r.battery_pct, 100);
        assert!(r.temp_c > 0.0);
        assert!(r.hour < 24);
    }

    #[test]
    fn read_returns_a_value() {
        // On non-Termux returns defaults; just confirm it doesn't panic.
        let r = SensorReading::read();
        assert!(r.battery_pct <= 100);
        assert!(r.hour < 24);
    }
}
