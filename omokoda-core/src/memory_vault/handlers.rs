use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
};

use crate::memory_vault::{
    types::{
        AccessLogQuery, CreateKnowledgeBody, KnowledgeTriple, ListDirQuery, SearchQuery,
        UpdateConfigBody, VaultStatus,
    },
    vault::MemoryVault,
};
use crate::server::AppState;

// ─── helper: resolve active agent ───────────────────────────────────────────

async fn active_agent(state: &AppState) -> Option<(String, String)> {
    let steward = state.steward.lock().await;
    let agent = steward.agent_core()?;
    Some((agent.id().as_str().to_string(), agent.name().to_string()))
}

// ─── GET /v1/vault ──────────────────────────────────────────────────────────

pub async fn get_vault_status(State(state): State<AppState>) -> impl IntoResponse {
    let Some((agent_id, agent_name)) = active_agent(&state).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no active agent"})),
        )
            .into_response();
    };
    let vault = MemoryVault::new(&agent_id, &agent_name, &state.vault_base);
    vault.log_access("status", "read", "owner");
    Json(VaultStatus {
        enabled: true,
        config: vault.load_config(),
        note_counts: vault.note_counts(),
        vault_path: vault.vault_path.to_string_lossy().to_string(),
    })
    .into_response()
}

// ─── GET /v1/vault/config ───────────────────────────────────────────────────

pub async fn get_vault_config(State(state): State<AppState>) -> impl IntoResponse {
    let Some((agent_id, agent_name)) = active_agent(&state).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no active agent"})),
        )
            .into_response();
    };
    let vault = MemoryVault::new(&agent_id, &agent_name, &state.vault_base);
    Json(vault.load_config()).into_response()
}

// ─── PUT /v1/vault/config ───────────────────────────────────────────────────

pub async fn put_vault_config(
    State(state): State<AppState>,
    Json(body): Json<UpdateConfigBody>,
) -> impl IntoResponse {
    let Some((agent_id, agent_name)) = active_agent(&state).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no active agent"})),
        )
            .into_response();
    };
    let vault = MemoryVault::new(&agent_id, &agent_name, &state.vault_base);
    let mut config = vault.load_config();
    config.access = body.access;
    if let Some(peers) = body.federation_peers {
        config.federation_peers = peers;
    }
    if let Some(auto) = body.auto_export {
        config.auto_export = auto;
    }
    match vault.save_config(&config) {
        Ok(_) => Json(config).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

// ─── POST /v1/vault/enable ──────────────────────────────────────────────────

pub async fn post_vault_enable(State(state): State<AppState>) -> impl IntoResponse {
    let Some((agent_id, agent_name)) = active_agent(&state).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no active agent"})),
        )
            .into_response();
    };
    let vault = MemoryVault::new(&agent_id, &agent_name, &state.vault_base);
    Json(serde_json::json!({
        "status": "enabled",
        "vault_path": vault.vault_path.to_string_lossy(),
    }))
    .into_response()
}

// ─── POST /v1/vault/sync ────────────────────────────────────────────────────

pub async fn post_vault_sync(State(state): State<AppState>) -> impl IntoResponse {
    let (agent_id, agent_name, session_clone) = {
        let steward = state.steward.lock().await;
        let Some(agent) = steward.agent_core() else {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "no active agent"})),
            )
                .into_response();
        };
        (
            agent.id().as_str().to_string(),
            agent.name().to_string(),
            agent.session().clone(),
        )
    };
    let vault = MemoryVault::new(&agent_id, &agent_name, &state.vault_base);
    vault.full_sync_from_session(&session_clone);
    Json(serde_json::json!({
        "status": "synced",
        "counts": vault.note_counts(),
    }))
    .into_response()
}

// ─── GET /v1/vault/galaxy ───────────────────────────────────────────────────

pub async fn get_galaxy_data(State(state): State<AppState>) -> impl IntoResponse {
    let Some((agent_id, agent_name)) = active_agent(&state).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no active agent"})),
        )
            .into_response();
    };
    let vault = MemoryVault::new(&agent_id, &agent_name, &state.vault_base);
    vault.log_access("galaxy", "read", "owner");
    Json(vault.get_galaxy_data()).into_response()
}

// ─── GET /v1/vault/search ───────────────────────────────────────────────────

pub async fn search_vault(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> impl IntoResponse {
    let Some((agent_id, agent_name)) = active_agent(&state).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no active agent"})),
        )
            .into_response();
    };
    let vault = MemoryVault::new(&agent_id, &agent_name, &state.vault_base);
    vault.log_access(&format!("search:{}", params.q), "search", "owner");
    Json(serde_json::json!({
        "query": params.q,
        "results": vault.search(&params.q),
    }))
    .into_response()
}

// ─── POST /v1/vault/knowledge ───────────────────────────────────────────────

pub async fn post_vault_knowledge(
    State(state): State<AppState>,
    Json(body): Json<CreateKnowledgeBody>,
) -> impl IntoResponse {
    let Some((agent_id, agent_name)) = active_agent(&state).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no active agent"})),
        )
            .into_response();
    };
    let vault = MemoryVault::new(&agent_id, &agent_name, &state.vault_base);
    let triple = KnowledgeTriple {
        subject: body.subject.clone(),
        predicate: body.predicate.clone(),
        object: body.object.clone(),
        confidence: body.confidence.unwrap_or(1.0),
        tags: body.tags.unwrap_or_default(),
    };
    vault.export_knowledge(&triple);
    vault.log_access(
        &format!(
            "knowledge:{}_{}_{}",
            body.subject, body.predicate, body.object
        ),
        "write",
        "owner",
    );
    Json(serde_json::json!({
        "status": "created",
        "triple": triple,
    }))
    .into_response()
}

// ─── GET /v1/vault/access-log ───────────────────────────────────────────────

pub async fn get_access_log(
    State(state): State<AppState>,
    Query(params): Query<AccessLogQuery>,
) -> impl IntoResponse {
    let Some((agent_id, agent_name)) = active_agent(&state).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no active agent"})),
        )
            .into_response();
    };
    let vault = MemoryVault::new(&agent_id, &agent_name, &state.vault_base);
    let limit = params.limit.unwrap_or(50);
    Json(serde_json::json!({
        "access_log": vault.get_access_log(limit),
    }))
    .into_response()
}

// ─── GET /v1/vault/file/*path ───────────────────────────────────────────────

pub async fn get_vault_file(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(file_path): Path<String>,
) -> impl IntoResponse {
    // Auth: if OMOKODA_COGNITION_TOKEN is set, require a matching Bearer token.
    // If the env var is unset (dev mode), allow through.
    if let Some(expected) = std::env::var("OMOKODA_COGNITION_TOKEN").ok().filter(|v| !v.is_empty()) {
        let presented = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));
        if presented != Some(expected.as_str()) {
            return (
                StatusCode::UNAUTHORIZED,
                "invalid or missing bearer token",
            )
                .into_response();
        }
    }
    let Some((agent_id, agent_name)) = active_agent(&state).await else {
        return (StatusCode::NOT_FOUND, "no active agent").into_response();
    };
    let vault = MemoryVault::new(&agent_id, &agent_name, &state.vault_base);
    vault.log_access(&file_path, "read", "owner");
    match vault.read_file(&file_path) {
        Ok(content) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/markdown; charset=utf-8")
            .header(
                header::CONTENT_DISPOSITION,
                format!(
                    "inline; filename=\"{}\"",
                    file_path.rsplit('/').next().unwrap_or("file.md")
                ),
            )
            .body(Body::from(content))
            .unwrap_or_else(|_| Response::new(Body::empty())),
        Err(e) if e == "not found" => (StatusCode::NOT_FOUND, "file not found").into_response(),
        Err(e) if e == "invalid path" => (StatusCode::FORBIDDEN, "invalid path").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

// ─── GET /v1/vault/ls ───────────────────────────────────────────────────────

pub async fn get_vault_ls(
    State(state): State<AppState>,
    Query(params): Query<ListDirQuery>,
) -> impl IntoResponse {
    let Some((agent_id, agent_name)) = active_agent(&state).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no active agent"})),
        )
            .into_response();
    };
    let vault = MemoryVault::new(&agent_id, &agent_name, &state.vault_base);
    let dir = params.dir.as_deref().unwrap_or("broadcasts");
    Json(serde_json::json!({
        "dir": dir,
        "files": vault.list_dir(dir),
    }))
    .into_response()
}

// ─── GET /v1/vault/download ─────────────────────────────────────────────────

pub async fn get_vault_download(State(state): State<AppState>) -> impl IntoResponse {
    let Some((agent_id, agent_name)) = active_agent(&state).await else {
        return (StatusCode::NOT_FOUND, "no active agent").into_response();
    };
    let vault_base = state.vault_base.clone();
    let bytes = tokio::task::spawn_blocking(move || {
        let vault = MemoryVault::new(&agent_id, &agent_name, &vault_base);
        vault.log_access(".", "download", "owner");
        vault.zip_vault()
    })
    .await;

    match bytes {
        Ok(Ok(data)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/zip")
            .header(
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"memory-vault.zip\"",
            )
            .body(Body::from(data))
            .unwrap_or_else(|_| Response::new(Body::empty())),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
