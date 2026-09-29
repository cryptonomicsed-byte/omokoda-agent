//! Habitat Phase 2 — four-predicate topology engine.
//!
//! Predicates a cognitive loop can query to decide whether to use a resource
//! or reach a peer. All checks are synchronous; live sensor reads happen
//! inside the resource store (not here).

use super::types::{HabitatAddress, PhysicalResource};

/// Can this agent REACH (communicate with) the given peer?
pub fn can_reach(my_addr: &HabitatAddress, peer_addr: &HabitatAddress) -> bool {
    // Same area → always reachable.
    if my_addr.area_id.is_some() && my_addr.area_id == peer_addr.area_id {
        return true;
    }
    // Both have mesh nodes → mesh reachability assumed.
    if my_addr.mesh_node.is_some() && peer_addr.mesh_node.is_some() {
        return true;
    }
    // Both have IPs → IP reachability assumed (no ping — design decision: avoid blocking IO here).
    if my_addr.ip.is_some() && peer_addr.ip.is_some() {
        return true;
    }
    // Both have Nostr npubs → reachable via relay.
    if my_addr.nostr_npub.is_some() && peer_addr.nostr_npub.is_some() {
        return true;
    }
    false
}

/// Can this agent OPERATE (has active power and compute) in the given area?
pub fn can_operate(area_resources: &[PhysicalResource]) -> bool {
    let has_power = area_resources
        .iter()
        .any(|r| r.kind == "power" && is_truthy(&r.last_value));
    let has_compute = area_resources
        .iter()
        .any(|r| r.kind == "storage" || r.kind == "sensor" || r.kind == "network");
    // At minimum, need either an explicit power resource OR at least one other resource.
    has_power || !area_resources.is_empty() && has_compute
}

/// IS the given resource AVAILABLE (not offline, not saturated)?
pub fn is_available(resource: &PhysicalResource) -> bool {
    // If we've never seen it → unavailable.
    let Some(seen_at) = resource.last_seen_at else {
        return false;
    };
    // Stale > 5 min → unavailable.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if now.saturating_sub(seen_at) > 300 {
        return false;
    }
    // If last value is explicitly "false" / 0 / "offline" → unavailable.
    if let Some(ref val) = resource.last_value {
        if is_falsy(val) {
            return false;
        }
    }
    true
}

/// IS the given area SAFE for autonomous operation?
/// Checks that no resource in the area is in a critical/alarm state.
pub fn is_safe(area_resources: &[PhysicalResource]) -> bool {
    !area_resources
        .iter()
        .any(|r| r.kind == "alarm" || is_alarm_value(&r.last_value))
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn is_truthy(v: &Option<serde_json::Value>) -> bool {
    match v {
        None => false,
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::Number(n)) => n.as_f64().unwrap_or(0.0) != 0.0,
        Some(serde_json::Value::String(s)) => !matches!(s.as_str(), "0" | "false" | "offline" | ""),
        Some(serde_json::Value::Null) => false,
        Some(_) => true,
    }
}

fn is_falsy(v: &serde_json::Value) -> bool {
    matches!(v, serde_json::Value::Bool(false) | serde_json::Value::Null)
        || matches!(v, serde_json::Value::String(s) if matches!(s.as_str(), "false" | "offline" | "0" | ""))
        || matches!(v, serde_json::Value::Number(n) if n.as_f64() == Some(0.0))
}

fn is_alarm_value(v: &Option<serde_json::Value>) -> bool {
    match v {
        Some(serde_json::Value::Bool(true)) => true,
        Some(serde_json::Value::String(s)) => matches!(s.as_str(), "alarm" | "critical" | "error"),
        _ => false,
    }
}
