//! Habitat Phase 1 — physical + digital presence types.
//!
//! Describes where an agent IS and what resources surround it, so the
//! cognitive loop can reason about physical availability, power, bandwidth,
//! and adjacent peers without coupling to any specific sensor API.

use gix_core::Gix1Index;
use gix_types::{Gix1, GixKind, GixNamespace, RoutingHints};
use serde::{Deserialize, Serialize};

/// A named physical or logical area the agent occupies or monitors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Area {
    /// Stable identifier — e.g. "node-room-a", "home-office", "field-site-3".
    pub area_id: String,
    /// Human-readable label.
    pub label: String,
    /// Optional GPS bounding box [lat_min, lon_min, lat_max, lon_max].
    pub bbox: Option<[f64; 4]>,
    /// Floor / floor-plan identifier for indoor spaces.
    pub floor: Option<String>,
    /// Custom metadata (building ID, provider region, zone type, etc.)
    pub meta: serde_json::Value,
}

/// A physical or virtual resource that the agent can sense or control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicalResource {
    /// Stable identifier.
    pub resource_id: String,
    /// Type tag — "sensor", "actuator", "network", "power", "storage", "peer".
    pub kind: String,
    /// Area this resource belongs to.
    pub area_id: String,
    /// Human-readable label.
    pub label: String,
    /// Last known value (unit is resource-type specific).
    pub last_value: Option<serde_json::Value>,
    /// Unix seconds of last update.
    pub last_seen_at: Option<u64>,
}

/// Composite habitat address — locates an agent in the physical-digital space.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HabitatAddress {
    /// Owning agent's id.
    pub agent_id: String,
    /// Current primary area.
    pub area_id: Option<String>,
    /// GPS coordinates (lat, lon) if available.
    pub gps: Option<(f64, f64)>,
    /// IP address of the local device.
    pub ip: Option<String>,
    /// Meshtastic node address if on-mesh.
    pub mesh_node: Option<String>,
    /// Nostr npub for this agent's public Nostr presence.
    pub nostr_npub: Option<String>,
    /// Unix seconds — when this address was last confirmed.
    pub confirmed_at: u64,
}

impl HabitatAddress {
    /// Serialize to `NetworkRepr { network: "habitat", address: "<area_id|agent_id>" }`.
    /// Satisfies acceptance criterion: AGENT_HABITAT_SPEC.md line 426.
    pub fn to_network_repr(&self) -> crate::bridge::NetworkRepr {
        let address = self
            .area_id
            .clone()
            .unwrap_or_else(|| self.agent_id.clone());
        crate::bridge::NetworkRepr {
            network: "habitat".to_string(),
            address,
        }
    }

    /// Expand into all known DIP network representations for an AgentManifest.
    /// Returns one entry per non-None transport (habitat, nostr, meshtastic, ip).
    /// Satisfies acceptance criterion: AGENT_HABITAT_SPEC.md line 427.
    pub fn to_agent_manifest_networks(&self) -> Vec<crate::bridge::NetworkRepr> {
        let mut nets = vec![self.to_network_repr()];
        if let Some(npub) = &self.nostr_npub {
            nets.push(crate::bridge::NetworkRepr {
                network: "nostr".to_string(),
                address: npub.clone(),
            });
        }
        if let Some(node) = &self.mesh_node {
            nets.push(crate::bridge::NetworkRepr {
                network: "meshtastic".to_string(),
                address: node.clone(),
            });
        }
        if let Some(ip) = &self.ip {
            nets.push(crate::bridge::NetworkRepr {
                network: "ip".to_string(),
                address: ip.clone(),
            });
        }
        nets
    }
}

/// Top-level Habitat: the agent's complete physical-digital presence.
///
/// Holds all areas and resources the agent has registered, plus its canonical
/// address and an optional OmoHome URL for the physical-world integration layer.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Habitat {
    pub habitat_id: String,
    pub agent_id: String,
    pub areas: Vec<Area>,
    pub resources: Vec<PhysicalResource>,
    pub home_address: HabitatAddress,
    /// URL of the agent's OmoHome (Home Assistant) instance, if any.
    pub omohome_url: Option<String>,
    /// GIX1 index — one envelope per registered area / upserted resource.
    #[serde(default)]
    pub gix1_index: Gix1Index,
}

impl Habitat {
    /// Create an empty habitat for the given agent.
    /// Reads `OMOHOME_URL` env var automatically.
    pub fn new(agent_id: &str) -> Self {
        Self {
            habitat_id: format!("habitat:{agent_id}"),
            agent_id: agent_id.to_string(),
            areas: Vec::new(),
            resources: Vec::new(),
            gix1_index: Gix1Index::new(),
            home_address: HabitatAddress {
                agent_id: agent_id.to_string(),
                confirmed_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                ..Default::default()
            },
            omohome_url: std::env::var("OMOHOME_URL").ok(),
        }
    }

    /// Register a new area (no-op if area_id already present).
    /// GIX Phase 3: stamps a `GixNamespace::OmokodaAgent` envelope on first registration.
    pub fn register_area(&mut self, area: Area) {
        if !self.areas.iter().any(|a| a.area_id == area.area_id) {
            let env = Gix1::new(
                GixKind::Physical,
                GixNamespace::OmokodaAgent,
                area.area_id.as_bytes(),
                None,
                now_ms(),
                RoutingHints::default(),
            );
            self.gix1_index.insert_gix1(env);
            self.areas.push(area);
        }
    }

    /// Upsert a physical resource (replace if resource_id already present).
    /// GIX Phase 3: stamps a `GixNamespace::OmokodaAgent` envelope on every upsert.
    pub fn upsert_resource(&mut self, resource: PhysicalResource) {
        let env = Gix1::new(
            GixKind::Physical,
            GixNamespace::OmokodaAgent,
            resource.resource_id.as_bytes(),
            None,
            now_ms(),
            RoutingHints::default(),
        );
        self.gix1_index.insert_gix1(env);
        if let Some(existing) = self
            .resources
            .iter_mut()
            .find(|r| r.resource_id == resource.resource_id)
        {
            *existing = resource;
        } else {
            self.resources.push(resource);
        }
    }

    /// All resources within a given area.
    pub fn resources_in(&self, area_id: &str) -> Vec<&PhysicalResource> {
        self.resources
            .iter()
            .filter(|r| r.area_id == area_id)
            .collect()
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
