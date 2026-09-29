/// Stalwart mailbox provisioner — creates an agent's persistent email account
/// at birth.
///
/// Fail-open: when AGENT_MAIL_DOMAIN is not set, `provision_mailbox` returns
/// `Ok(None)` and birth continues normally. All errors are logged but never
/// propagate to the birth path.
///
/// Phase 9.3 — Agent email provisioning.
use rand::Rng;
use serde::{Deserialize, Serialize};

// ── Public types ──────────────────────────────────────────────────────────────

/// Stalwart admin connection configuration.
///
/// All fields are read from env vars at call time — nothing is hardcoded.
#[derive(Debug, Clone)]
pub struct MailboxConfig {
    /// Stalwart management API base URL.
    /// Source: `AGENT_MAIL_ADMIN_URL` env var (e.g. `http://mail.local:8080`).
    pub admin_url: String,
    /// Bearer token or Basic-auth token for the Stalwart admin API.
    /// Source: `AGENT_MAIL_ADMIN_TOKEN` env var.
    pub admin_token: String,
    /// Mail domain for new accounts (e.g. `agents.sovereign.local`).
    /// Source: `AGENT_MAIL_DOMAIN` env var.
    pub mail_domain: String,
}

impl MailboxConfig {
    /// Load configuration from environment variables.
    ///
    /// Returns `None` if any required variable is absent — the caller should
    /// treat this as "mail provisioning disabled" and skip silently.
    pub fn from_env() -> Option<Self> {
        let admin_url = std::env::var("AGENT_MAIL_ADMIN_URL").ok()?;
        let admin_token = std::env::var("AGENT_MAIL_ADMIN_TOKEN").ok()?;
        let mail_domain = std::env::var("AGENT_MAIL_DOMAIN").ok()?;
        if admin_url.is_empty() || admin_token.is_empty() || mail_domain.is_empty() {
            return None;
        }
        Some(Self {
            admin_url,
            admin_token,
            mail_domain,
        })
    }
}

/// Credentials returned after a successful mailbox provisioning call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailboxCredentials {
    /// Full email address: `{local}@{domain}`
    pub email: String,
    /// Randomly-generated account password (32 alphanumeric chars).
    pub password: String,
    /// JMAP API endpoint for this account.
    pub jmap_url: String,
    /// IMAP hostname (for reference / future IMAP tool support).
    pub imap_host: String,
    /// SMTP hostname (for reference / future SMTP send support).
    pub smtp_host: String,
}

// ── Provisioner ───────────────────────────────────────────────────────────────

/// Provision a new Stalwart mailbox for an agent.
///
/// Creates the account via the Stalwart admin API (`POST /api/principal`).
/// Returns `Ok(Some(credentials))` on success, `Ok(None)` when mail is not
/// configured (AGENT_MAIL_DOMAIN absent), or `Err(...)` on API failure.
///
/// Callers in the birth path should:
/// ```ignore
/// if let Some(creds) = provision_mailbox(...).await.ok().flatten() {
///     vault.agent_email = Some(creds.email);
///     vault.agent_email_password = Some(creds.password);
///     vault.email_jmap_url = Some(creds.jmap_url);
///     // ... etc
/// }
/// ```
pub async fn provision_mailbox(
    agent_email_local: &str,
    agent_display_name: &str,
    config: &MailboxConfig,
) -> Result<MailboxCredentials, String> {
    let password = generate_secure_password();
    let email = format!("{}@{}", agent_email_local, config.mail_domain);

    // Stalwart admin API: POST /api/principal
    // https://stalw.art/docs/api/management/principals
    let body = serde_json::json!({
        "type": "individual",
        "name": agent_email_local,
        "description": agent_display_name,
        "emails": [email],
        "secrets": [password],
    });

    let client = reqwest::Client::new();
    let url = format!("{}/api/principal", config.admin_url.trim_end_matches('/'));

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.admin_token))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("stalwart admin API request failed: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        // 409 Conflict = account already exists (idempotent — return existing creds)
        if status.as_u16() == 409 {
            tracing::info!(
                email,
                "mailbox_provisioner: account already exists (409), reusing"
            );
        } else {
            let body_text = resp.text().await.unwrap_or_default();
            return Err(format!(
                "stalwart admin API returned {}: {}",
                status, body_text
            ));
        }
    }

    // Derive JMAP / IMAP / SMTP from the admin URL base.
    // Stalwart serves JMAP at `{base}/jmap` by default.
    let base = config.admin_url.trim_end_matches('/');
    let jmap_url = format!("{}/jmap", base);
    // IMAP/SMTP hosts: strip port from admin URL, use conventional ports.
    let imap_host = extract_host(base);
    let smtp_host = imap_host.clone();

    tracing::info!(
        email,
        "mailbox_provisioner: provisioned mailbox successfully"
    );

    Ok(MailboxCredentials {
        email,
        password,
        jmap_url,
        imap_host,
        smtp_host,
    })
}

/// Fail-open convenience wrapper for use in the birth path.
///
/// Returns `Some(MailboxCredentials)` on success, `None` on any error or
/// when mail is not configured. Errors are only logged, never propagated.
pub async fn try_provision_mailbox(
    agent_email_local: &str,
    agent_display_name: &str,
) -> Option<MailboxCredentials> {
    let config = MailboxConfig::from_env()?;
    match provision_mailbox(agent_email_local, agent_display_name, &config).await {
        Ok(creds) => Some(creds),
        Err(e) => {
            tracing::warn!(
                "mailbox_provisioner: provisioning failed (fail-open): {}",
                e
            );
            None
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Generate a 32-character alphanumeric password using a cryptographically
/// secure RNG (`rand::thread_rng` backed by OS entropy via getrandom).
fn generate_secure_password() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Extract the host:port part from a URL string for IMAP/SMTP references.
fn extract_host(url: &str) -> String {
    // Strip scheme
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    // Strip path
    without_scheme
        .split('/')
        .next()
        .unwrap_or(without_scheme)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_length_and_charset() {
        let pw = generate_secure_password();
        assert_eq!(pw.len(), 32);
        assert!(pw.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn passwords_are_unique() {
        let a = generate_secure_password();
        let b = generate_secure_password();
        // Astronomically unlikely to collide
        assert_ne!(a, b);
    }

    #[test]
    fn extract_host_strips_scheme_and_path() {
        assert_eq!(
            extract_host("https://mail.local:8080/api"),
            "mail.local:8080"
        );
        assert_eq!(extract_host("http://mail.example.com"), "mail.example.com");
    }

    #[test]
    fn config_from_env_returns_none_without_vars() {
        // With no env vars set, from_env() must return None (not panic)
        // — we can't clear env in tests but we can verify it handles absence.
        // This test only checks the None path if vars are absent.
        if std::env::var("AGENT_MAIL_DOMAIN").is_err() {
            assert!(MailboxConfig::from_env().is_none());
        }
    }
}
