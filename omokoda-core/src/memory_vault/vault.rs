use crate::memory_vault::types::{
    AccessLevel, AccessLogEntry, Edge, GalaxyBounds, GalaxyData, KnowledgeTriple, Nebula,
    SearchResult, Star, VaultConfig, VaultDirEntry,
};
use crate::session::{ContentBlock, Session};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct MemoryVault {
    pub agent_id: String,
    pub agent_name: String,
    pub vault_path: PathBuf,
}

impl MemoryVault {
    pub fn new(agent_id: &str, agent_name: &str, base_dir: &Path) -> Self {
        let vault_path = base_dir.join("vaults").join(agent_id);
        let _ = fs::create_dir_all(&vault_path);
        for sub in &[
            "broadcasts",
            "knowledge",
            "traces",
            "drafts",
            "templates",
            ".vault",
        ] {
            let _ = fs::create_dir_all(vault_path.join(sub));
        }
        let vault = Self {
            agent_id: agent_id.to_string(),
            agent_name: agent_name.to_string(),
            vault_path,
        };
        vault.seed_templates();
        vault
    }

    fn seed_templates(&self) {
        let tmpl = self
            .vault_path
            .join("templates")
            .join("broadcast-template.md");
        if !tmpl.exists() {
            let _ = fs::write(
                &tmpl,
                "---\ntype: star\ncontent_type: text\ntags: []\n---\n\n# Broadcast Title\n\n\
                Write your broadcast here.\n\n## Key Points\n\n- Point 1\n- Point 2\n\n\
                ## Tags\n\n#knowledge #insight\n",
            );
        }
    }

    // ─── config ─────────────────────────────────────────────────────────────

    pub fn load_config(&self) -> VaultConfig {
        let config_path = self.vault_path.join(".vault").join("config.json");
        if let Ok(content) = fs::read_to_string(&config_path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            let default = VaultConfig::default();
            let _ = self.save_config(&default);
            default
        }
    }

    pub fn save_config(&self, config: &VaultConfig) -> Result<(), String> {
        let config_path = self.vault_path.join(".vault").join("config.json");
        let json =
            serde_json::to_string_pretty(config).map_err(|e| format!("serialize config: {e}"))?;
        fs::write(&config_path, json).map_err(|e| format!("write config: {e}"))
    }

    // ─── access log ─────────────────────────────────────────────────────────

    pub fn log_access(&self, resource: &str, access_type: &str, accessor: &str) {
        let entry = AccessLogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            resource: resource.to_string(),
            access_type: access_type.to_string(),
            accessor: accessor.to_string(),
        };
        let log_path = self.vault_path.join(".vault").join("access_log.jsonl");
        if let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            if let Ok(line) = serde_json::to_string(&entry) {
                let _ = writeln!(file, "{}", line);
            }
        }
    }

    pub fn get_access_log(&self, limit: usize) -> Vec<AccessLogEntry> {
        let log_path = self.vault_path.join(".vault").join("access_log.jsonl");
        let Ok(content) = fs::read_to_string(&log_path) else {
            return vec![];
        };
        let mut entries: Vec<AccessLogEntry> = content
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        let skip = entries.len().saturating_sub(limit);
        entries.drain(..skip);
        entries.into_iter().rev().collect()
    }

    // ─── spatial index ──────────────────────────────────────────────────────

    fn write_spatial_index(&self, galaxy: &GalaxyData) -> Result<(), String> {
        let index = serde_json::json!({
            "agent_id": galaxy.agent_id,
            "agent_name": galaxy.agent_name,
            "stars": galaxy.stars.iter().map(|s| serde_json::json!({
                "id": s.id, "path": s.path, "x": s.x, "y": s.y, "z": s.z,
            })).collect::<Vec<_>>(),
            "edges": galaxy.edges.iter().map(|e| serde_json::json!({
                "id": e.id, "path": e.path,
                "source": e.source, "target": e.target,
            })).collect::<Vec<_>>(),
            "nebulae": galaxy.nebulae.iter().map(|n| serde_json::json!({
                "id": n.id, "path": n.path, "x": n.x, "y": n.y, "z": n.z,
            })).collect::<Vec<_>>(),
        });
        let json = serde_json::to_string_pretty(&index)
            .map_err(|e| format!("serialize spatial index: {e}"))?;
        fs::write(
            self.vault_path.join(".vault").join("spatial_index.json"),
            json,
        )
        .map_err(|e| format!("write spatial index: {e}"))
    }

    // ─── note writing ────────────────────────────────────────────────────────

    fn write_note(
        &self,
        category: &str,
        filename: &str,
        frontmatter: &HashMap<String, serde_json::Value>,
        body: &str,
    ) {
        let path = self.vault_path.join(category).join(filename);
        let fm_lines: Vec<String> = frontmatter
            .iter()
            .map(|(k, v)| format!("{}: {}", k, yaml_value(v)))
            .collect();
        let content = format!("---\n{}\n---\n\n{}", fm_lines.join("\n"), body);
        let _ = fs::write(&path, content);
    }

    // ─── knowledge triples ───────────────────────────────────────────────────

    pub fn export_knowledge(&self, triple: &KnowledgeTriple) {
        let source_coords = self.spatial_hash(&triple.subject, "knowledge");
        let target_coords = self.spatial_hash(&triple.object, "knowledge");

        let mut fm: HashMap<String, serde_json::Value> = HashMap::new();
        let id = format!(
            "knowledge_{}_{}_{}",
            triple.subject, triple.predicate, triple.object
        );
        fm.insert("id".into(), serde_json::json!(sanitize_filename(&id)));
        fm.insert("type".into(), serde_json::json!("edge"));
        fm.insert("subject".into(), serde_json::json!(&triple.subject));
        fm.insert("predicate".into(), serde_json::json!(&triple.predicate));
        fm.insert("object".into(), serde_json::json!(&triple.object));
        fm.insert("confidence".into(), serde_json::json!(triple.confidence));
        fm.insert("tags".into(), serde_json::json!(triple.tags));
        fm.insert("galaxy_source_x".into(), serde_json::json!(source_coords.0));
        fm.insert("galaxy_source_y".into(), serde_json::json!(source_coords.1));
        fm.insert("galaxy_source_z".into(), serde_json::json!(source_coords.2));
        fm.insert("galaxy_target_x".into(), serde_json::json!(target_coords.0));
        fm.insert("galaxy_target_y".into(), serde_json::json!(target_coords.1));
        fm.insert("galaxy_target_z".into(), serde_json::json!(target_coords.2));
        fm.insert("galaxy_weight".into(), serde_json::json!(triple.confidence));
        fm.insert(
            "created".into(),
            serde_json::json!(chrono::Utc::now().to_rfc3339()),
        );

        let tags_json = serde_json::to_string(&triple.tags).unwrap_or_default();
        let body = format!(
            "# {} → {} → {}\n\nConfidence: {}\n\nThis knowledge edge connects `{}` to `{}` \
through the relationship `{}`.\n\n## Context\n{}\n",
            triple.subject,
            triple.predicate,
            triple.object,
            triple.confidence,
            triple.subject,
            triple.object,
            triple.predicate,
            tags_json,
        );

        let safe_name = sanitize_filename(&format!(
            "{}_{}_{}",
            triple.subject, triple.predicate, triple.object
        ));
        self.write_note("knowledge", &format!("{}.md", safe_name), &fm, &body);
    }

    // ─── export helpers ──────────────────────────────────────────────────────

    pub fn export_think(&self, content: &str, timestamp: u64, idx: usize) {
        let dt =
            chrono::DateTime::from_timestamp(timestamp as i64, 0).unwrap_or_else(chrono::Utc::now);
        let date_str = dt.format("%Y-%m-%d").to_string();
        let coords = self.spatial_hash(content, "trace");

        let mut fm: HashMap<String, serde_json::Value> = HashMap::new();
        fm.insert(
            "id".into(),
            serde_json::json!(format!("trace_{}_{}", timestamp, idx)),
        );
        fm.insert("type".into(), serde_json::json!("nebula"));
        fm.insert("trace_type".into(), serde_json::json!("thought"));
        fm.insert("galaxy_x".into(), serde_json::json!(coords.0));
        fm.insert("galaxy_y".into(), serde_json::json!(coords.1));
        fm.insert("galaxy_z".into(), serde_json::json!(coords.2));
        fm.insert("galaxy_opacity".into(), serde_json::json!(0.2));
        fm.insert(
            "galaxy_size".into(),
            serde_json::json!((content.len() as f64 / 10.0).min(100.0)),
        );
        fm.insert("galaxy_color".into(), serde_json::json!("#663399"));
        fm.insert("created".into(), serde_json::json!(dt.to_rfc3339()));

        let preview: String = content.chars().take(500).collect();
        let body = format!(
            "# Ghost Trace: thought\n\n> {}\n\n## Metadata\n- Type: thought\n- Index: {}\n",
            preview, idx
        );
        let filename = format!("{}-trace-{}.md", date_str, idx);
        self.write_note("traces", &filename, &fm, &body);
    }

    pub fn export_message(&self, msg: &crate::session::ConversationMessage, idx: usize) {
        let text: String = msg
            .blocks
            .iter()
            .filter_map(|b| {
                if let ContentBlock::Text { text } = b {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        if text.is_empty() {
            return;
        }

        let role = format!("{:?}", msg.role).to_lowercase();
        let coords = self.spatial_hash(&text, "broadcast");
        let dt = chrono::DateTime::from_timestamp(msg.timestamp as i64, 0)
            .unwrap_or_else(chrono::Utc::now);
        let date_str = dt.format("%Y-%m-%d").to_string();
        let title: String = text
            .lines()
            .next()
            .unwrap_or("Untitled")
            .chars()
            .take(60)
            .collect();

        let mut fm: HashMap<String, serde_json::Value> = HashMap::new();
        fm.insert("id".into(), serde_json::json!(format!("msg_{}", idx)));
        fm.insert("type".into(), serde_json::json!("star"));
        fm.insert("content_type".into(), serde_json::json!("text"));
        fm.insert("role".into(), serde_json::json!(&role));
        fm.insert("tags".into(), serde_json::json!([&role]));
        fm.insert("galaxy_x".into(), serde_json::json!(coords.0));
        fm.insert("galaxy_y".into(), serde_json::json!(coords.1));
        fm.insert("galaxy_z".into(), serde_json::json!(coords.2));
        fm.insert("galaxy_size".into(), serde_json::json!(8.0_f64));
        fm.insert(
            "galaxy_color".into(),
            serde_json::json!(content_color("text")),
        );
        fm.insert("constellation".into(), serde_json::json!(&role));
        fm.insert("created".into(), serde_json::json!(dt.to_rfc3339()));

        let body_text: String = text.chars().take(2000).collect();
        let body = format!("# {}\n\n{}\n", title, body_text);
        let safe_title = sanitize_filename(&title);
        let filename = format!("{}-{}-{}.md", date_str, safe_title, idx);
        self.write_note("broadcasts", &filename, &fm, &body);
    }

    // ─── sync ────────────────────────────────────────────────────────────────

    pub fn full_sync_from_session(&self, session: &Session) {
        for (idx, msg) in session.public_messages.iter().enumerate() {
            self.export_message(msg, idx);
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        for (idx, think) in session.think_history.iter().enumerate() {
            self.export_think(think, now, idx);
        }

        let mut config = self.load_config();
        config.last_synced = Some(chrono::Utc::now().to_rfc3339());
        let _ = self.save_config(&config);
        let _ = self.write_readme(&config, session.public_messages.len());

        let galaxy = self.get_galaxy_data();
        let _ = self.write_spatial_index(&galaxy);

        self.log_access(".", "sync", "owner");
    }

    fn write_readme(&self, config: &VaultConfig, message_count: usize) -> Result<(), String> {
        let broadcasts = count_md_dir(&self.vault_path.join("broadcasts"));
        let knowledge = count_md_dir(&self.vault_path.join("knowledge"));
        let traces = count_md_dir(&self.vault_path.join("traces"));
        let fed = if config.federation_peers.is_empty() {
            "No federated peers configured.".to_string()
        } else {
            format!("Allowed peers: {}", config.federation_peers.join(", "))
        };

        let readme = format!(
            "---\ntype: galaxy_map\nagent: {name}\nagent_id: {id}\naccess: {access}\nlast_synced: {synced}\n---\n\n\
# 🌌 {name}'s Memory Galaxy\n\n\
> *\"Every broadcast is a star. Every thought is a nebula.\"*\n\n\
## Privacy Setting\n🔒 **{access_upper}** — {access_desc}\n\n\
## Navigation\n\
| Region | Count | Description |\n\
|--------|-------|-------------|\n\
| [[broadcasts/\\|📡 Broadcasts]] | {broadcasts} | Published content (stars) |\n\
| [[knowledge/\\|🔗 Knowledge]] | {knowledge} | Triples (constellation edges) |\n\
| [[traces/\\|👁 Ghost Traces]] | {traces} | Thought nebulae |\n\
| [[drafts/\\|📝 Drafts]] | — | Unpublished work |\n\
| messages | {message_count} | Session messages synced |\n\n\
## Galaxy Controls\n\
- **Zoom**: Scroll or pinch\n\
- **Pan**: Drag\n\
- **Filter**: Click constellation names\n\
- **Search**: Use vault search\n\n\
## Federation\n{fed}\n",
            name = self.agent_name,
            id = self.agent_id,
            access = config.access,
            synced = config.last_synced.as_deref().unwrap_or("never"),
            access_upper = config.access.to_string().to_uppercase(),
            access_desc = access_description(&config.access),
            broadcasts = broadcasts,
            knowledge = knowledge,
            traces = traces,
            message_count = message_count,
            fed = fed,
        );

        fs::write(self.vault_path.join("README.md"), readme)
            .map_err(|e| format!("write README: {e}"))
    }

    // ─── galaxy data ─────────────────────────────────────────────────────────

    pub fn get_galaxy_data(&self) -> GalaxyData {
        let mut stars = Vec::new();
        let mut edges = Vec::new();
        let mut nebulae = Vec::new();

        // broadcasts → stars
        if let Ok(entries) = fs::read_dir(self.vault_path.join("broadcasts")) {
            for entry in entries.flatten() {
                if !is_md(&entry) {
                    continue;
                }
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    let fm = self.parse_frontmatter(&content);
                    if fm.get("type").map(String::as_str) == Some("star") {
                        stars.push(Star {
                            id: fm.get("id").cloned().unwrap_or_default(),
                            title: extract_title(&content),
                            x: parse_f64(&fm, "galaxy_x"),
                            y: parse_f64(&fm, "galaxy_y"),
                            z: parse_f64(&fm, "galaxy_z"),
                            size: parse_f64_or(&fm, "galaxy_size", 10.0),
                            color: fm
                                .get("galaxy_color")
                                .cloned()
                                .unwrap_or_else(|| "#ffffff".into()),
                            constellation: fm
                                .get("constellation")
                                .cloned()
                                .unwrap_or_else(|| "unknown".into()),
                            tags: fm
                                .get("tags")
                                .map(|t| {
                                    serde_json::from_str::<Vec<String>>(t)
                                        .unwrap_or_else(|_| vec![t.clone()])
                                })
                                .unwrap_or_default(),
                            content_type: fm
                                .get("content_type")
                                .cloned()
                                .unwrap_or_else(|| "text".into()),
                            path: filename_str(&entry),
                        });
                    }
                }
            }
        }

        // knowledge → edges
        if let Ok(entries) = fs::read_dir(self.vault_path.join("knowledge")) {
            for entry in entries.flatten() {
                if !is_md(&entry) {
                    continue;
                }
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    let fm = self.parse_frontmatter(&content);
                    if fm.get("type").map(String::as_str) == Some("edge") {
                        edges.push(Edge {
                            id: fm.get("id").cloned().unwrap_or_default(),
                            subject: fm.get("subject").cloned().unwrap_or_default(),
                            predicate: fm.get("predicate").cloned().unwrap_or_default(),
                            object: fm.get("object").cloned().unwrap_or_default(),
                            source: [
                                parse_f64(&fm, "galaxy_source_x"),
                                parse_f64(&fm, "galaxy_source_y"),
                                parse_f64(&fm, "galaxy_source_z"),
                            ],
                            target: [
                                parse_f64(&fm, "galaxy_target_x"),
                                parse_f64(&fm, "galaxy_target_y"),
                                parse_f64(&fm, "galaxy_target_z"),
                            ],
                            weight: parse_f64_or(&fm, "galaxy_weight", 1.0),
                            path: filename_str(&entry),
                        });
                    }
                }
            }
        }

        // traces → nebulae
        if let Ok(entries) = fs::read_dir(self.vault_path.join("traces")) {
            for entry in entries.flatten() {
                if !is_md(&entry) {
                    continue;
                }
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    let fm = self.parse_frontmatter(&content);
                    if fm.get("type").map(String::as_str) == Some("nebula") {
                        nebulae.push(Nebula {
                            id: fm.get("id").cloned().unwrap_or_default(),
                            trace_type: fm
                                .get("trace_type")
                                .cloned()
                                .unwrap_or_else(|| "thought".into()),
                            x: parse_f64(&fm, "galaxy_x"),
                            y: parse_f64(&fm, "galaxy_y"),
                            z: parse_f64(&fm, "galaxy_z"),
                            opacity: parse_f64_or(&fm, "galaxy_opacity", 0.2),
                            size: parse_f64_or(&fm, "galaxy_size", 50.0),
                            path: filename_str(&entry),
                        });
                    }
                }
            }
        }

        let mut clusters: HashMap<String, Vec<Star>> = HashMap::new();
        for star in &stars {
            clusters
                .entry(star.constellation.clone())
                .or_default()
                .push(star.clone());
        }

        GalaxyData {
            agent_name: self.agent_name.clone(),
            agent_id: self.agent_id.clone(),
            stars,
            edges,
            nebulae,
            clusters,
            bounds: GalaxyBounds {
                min: [0.0, 0.0, 0.0],
                max: [8000.0, 1000.0, 500.0],
            },
        }
    }

    // ─── search ──────────────────────────────────────────────────────────────

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let q = query.to_lowercase();
        let mut results = Vec::new();
        for dir in &["broadcasts", "knowledge", "traces"] {
            if let Ok(entries) = fs::read_dir(self.vault_path.join(dir)) {
                for entry in entries.flatten() {
                    if !is_md(&entry) {
                        continue;
                    }
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        if content.to_lowercase().contains(&q) {
                            results.push(SearchResult {
                                path: format!("{}/{}", dir, filename_str(&entry)),
                                title: extract_title(&content),
                                snippet: find_snippet(&content, &q),
                            });
                        }
                    }
                }
            }
        }
        results
    }

    // ─── counts ──────────────────────────────────────────────────────────────

    pub fn note_counts(&self) -> HashMap<String, usize> {
        let mut m = HashMap::new();
        for dir in &["broadcasts", "knowledge", "traces", "drafts"] {
            m.insert((*dir).to_string(), count_md_dir(&self.vault_path.join(dir)));
        }
        m
    }

    // ─── directory listing ───────────────────────────────────────────────────

    pub fn list_dir(&self, dir: &str) -> Vec<VaultDirEntry> {
        let allowed = ["broadcasts", "knowledge", "traces", "drafts", "templates"];
        if !allowed.contains(&dir) {
            return vec![];
        }
        let dir_path = self.vault_path.join(dir);
        let Ok(entries) = fs::read_dir(&dir_path) else {
            return vec![];
        };
        let mut files: Vec<VaultDirEntry> = entries
            .flatten()
            .filter(is_md)
            .map(|e| {
                let name = filename_str(&e);
                VaultDirEntry {
                    path: format!("{}/{}", dir, name),
                    name,
                }
            })
            .collect();
        files.sort_by(|a, b| a.name.cmp(&b.name));
        files
    }

    // ─── file access ─────────────────────────────────────────────────────────

    pub fn read_file(&self, rel_path: &str) -> Result<String, String> {
        // Reject path components that could escape the vault root
        let rel = std::path::Path::new(rel_path);
        if rel.components().any(|c| matches!(c,
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_)
        )) {
            return Err("invalid path".to_string());
        }
        let full = self.vault_path.join(rel);
        // Canonicalize-based containment check for existing paths
        let canon_vault = self.vault_path.canonicalize()
            .map_err(|e| e.to_string())?;
        if full.exists() {
            let canon_full = full.canonicalize().map_err(|e| e.to_string())?;
            if !canon_full.starts_with(&canon_vault) {
                return Err("invalid path".to_string());
            }
        }
        if !full.exists() || !full.is_file() {
            return Err("not found".to_string());
        }
        fs::read_to_string(&full).map_err(|e| e.to_string())
    }

    // ─── zip download ────────────────────────────────────────────────────────

    pub fn zip_vault(&self) -> Result<Vec<u8>, String> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for entry in walkdir::WalkDir::new(&self.vault_path).sort_by_file_name() {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let Ok(rel) = path.strip_prefix(&self.vault_path) else {
                continue;
            };
            let rel_str = rel.to_string_lossy();
            if rel_str.is_empty() {
                continue;
            }
            if path.is_dir() {
                zip.add_directory(format!("{}/", rel_str), options)
                    .map_err(|e| e.to_string())?;
            } else {
                zip.start_file(rel_str.as_ref(), options)
                    .map_err(|e| e.to_string())?;
                let data = fs::read(path).map_err(|e| e.to_string())?;
                zip.write_all(&data).map_err(|e| e.to_string())?;
            }
        }

        let cursor = zip.finish().map_err(|e| e.to_string())?;
        Ok(cursor.into_inner())
    }

    // ─── spatial hashing ─────────────────────────────────────────────────────

    fn spatial_hash(&self, seed: &str, category: &str) -> (f64, f64, f64) {
        let input = format!("{}:{}:{}", seed, category, self.agent_id);
        let hash = Sha256::digest(input.as_bytes());
        let offsets = [
            ("broadcast", 0.0_f64),
            ("knowledge", 2000.0),
            ("trace", 4000.0),
            ("draft", 6000.0),
        ];
        let off = offsets
            .iter()
            .find(|(k, _)| *k == category)
            .map(|(_, v)| *v)
            .unwrap_or(0.0);
        let x = (u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]) % 1000) as f64 + off;
        let y = (u32::from_be_bytes([hash[4], hash[5], hash[6], hash[7]]) % 1000) as f64;
        let z = (u32::from_be_bytes([hash[8], hash[9], hash[10], hash[11]]) % 500) as f64;
        (x, y, z)
    }

    fn parse_frontmatter(&self, content: &str) -> HashMap<String, String> {
        let mut result = HashMap::new();
        if !content.starts_with("---") {
            return result;
        }
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() < 3 {
            return result;
        }
        for line in parts[1].trim().lines() {
            if let Some(pos) = line.find(':') {
                let key = line[..pos].trim().to_string();
                let val = line[pos + 1..].trim().to_string();
                result.insert(key, val);
            }
        }
        result
    }
}

// ── free helpers ─────────────────────────────────────────────────────────────

fn yaml_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Array(arr) => serde_json::to_string(arr).unwrap_or_default(),
        other => other.to_string(),
    }
}

fn count_md_dir(path: &PathBuf) -> usize {
    fs::read_dir(path)
        .map(|e| e.flatten().filter(is_md).count())
        .unwrap_or(0)
}

fn is_md(entry: &fs::DirEntry) -> bool {
    entry
        .path()
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e == "md")
        .unwrap_or(false)
}

fn filename_str(entry: &fs::DirEntry) -> String {
    entry
        .path()
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

fn extract_title(content: &str) -> String {
    for line in content.lines() {
        if let Some(stripped) = line.strip_prefix("# ") {
            return stripped.trim().to_string();
        }
    }
    "Untitled".to_string()
}

fn find_snippet(content: &str, query: &str) -> String {
    let lower = content.to_lowercase();
    if let Some(pos) = lower.find(query) {
        let start = pos.saturating_sub(60);
        let end = (pos + query.len() + 60).min(content.len());
        let start = content
            .char_indices()
            .map(|(i, _)| i)
            .find(|&i| i >= start)
            .unwrap_or(0);
        let end = content
            .char_indices()
            .map(|(i, _)| i)
            .rfind(|&i| i <= end)
            .unwrap_or(content.len());
        format!("...{}...", &content[start..end])
    } else {
        content.chars().take(120).collect()
    }
}

pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(50)
        .collect()
}

fn content_color(ct: &str) -> &'static str {
    match ct {
        "video" => "#ff6b6b",
        "audio" => "#4ecdc4",
        "image" => "#a8e6cf",
        "graph" => "#c7ceea",
        "debate" => "#ff8b94",
        _ => "#ffe66d",
    }
}

fn access_description(access: &AccessLevel) -> &'static str {
    match access {
        AccessLevel::Private => "Only you can access this vault.",
        AccessLevel::Followers => "Verified followers can view your galaxy.",
        AccessLevel::Federated => "Followers and whitelisted federation peers can access.",
        AccessLevel::Public => "Open to all. Your galaxy is a public knowledge resource.",
    }
}

fn parse_f64(fm: &HashMap<String, String>, key: &str) -> f64 {
    fm.get(key).and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

fn parse_f64_or(fm: &HashMap<String, String>, key: &str, default: f64) -> f64 {
    fm.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
}
