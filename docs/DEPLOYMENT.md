# Ọ̀mọ Kọ́dà Agent OS — Deployment Guide

This document lists every environment variable that gates production functionality.
All are **fail-open** (the feature is skipped / degraded when the var is unset)
unless marked **REQUIRED** (the process will refuse to start or a gate will always fail).

---

## Core Identity

| Variable | Default | Notes |
|---|---|---|
| `CHAIN_ID` | `mainnet` | Chain label stamped on receipts. Set `testnet` for dev. |
| `OMOKODA_ADMIN_TOKEN` | — | Bearer token for admin endpoints. Set to a secret string. |
| `OMOKODA_COGNITION_TOKEN` | — | Token for `/v1/think` and `/v1/act` calls. |
| `OMOKODA_HEARTBEAT_KEY` | — | Base64url-encoded 32-byte Ed25519 scalar. When set, every heartbeat beat is Ed25519-signed. |

## Walrus Blob Store (W-10 — required for production memory anchoring)

| Variable | Default | Notes |
|---|---|---|
| `WALRUS_PUBLISHER_URL` | — | e.g. `https://publisher.walrus-testnet.walrus.space` — **set this in production** |
| `WALRUS_AGGREGATOR_URL` | — | e.g. `https://aggregator.walrus-testnet.walrus.space` — **set this in production** |
| `WALRUS_EPOCHS` | `5` | Blob storage duration in epochs. |

Without `WALRUS_PUBLISHER_URL`, birth memory is not anchored to Walrus. The agent still works (fail-open) but provenance is local only.

## Seal / TEE Envelope (W-10 — required for encrypted memory in production)

| Variable | Default | Notes |
|---|---|---|
| `OMOKODA_TEE_SEAL` | `0` | Set to `1` to enable TEE memory envelope. **Required for encrypted private memory.** |
| `NAUTILUS_SEAL_KEY` | — | Seal key for the TEE bridge. |

Without `OMOKODA_TEE_SEAL=1`, private memory is stored unencrypted inside the vault (machine-vault still protects it, but no TEE wrapper).

## Zàngbétò Enforcement

| Variable | Default | Notes |
|---|---|---|
| `ZANGBETO_URL` | — | e.g. `http://localhost:7780` — enforcement bridge. When set, post-act audits make real HTTP calls; when unset, SHA-256 hash fallback (always passes). |

## Vantage Hub

| Variable | Default | Notes |
|---|---|---|
| `VANTAGE_URL` | — | e.g. `https://vantage.example.com` |
| `VANTAGE_API_KEY` or `VANTAGE_KEY` | — | API key for this agent's Vantage account. |
| `VANTAGE_AGENT_ID` | — | Agent ID registered on Vantage (matches birth DNA fingerprint). |
| `VANTAGE_GUILD_SLUG` | — | Guild/organization slug. |

## Nostr Identity

| Variable | Default | Notes |
|---|---|---|
| `NOSTR_PRIVKEY` | — | Hex or nsec-encoded Nostr private key. Derived from mnemonic at birth if unset. |
| `NOSTR_RELAYS` | — | Comma-separated relay URLs. |
| `AGENT_NOSTR_RELAYS` | — | Per-agent relay override (takes precedence over `NOSTR_RELAYS`). |
| `BUZZ_RELAY_URL` | — | Relay URL for ACP (Agent Communication Protocol) messages. |
| `BUZZ_PRIVATE_KEY` | — | Private key for Buzz/ACP identity. |
| `IP_LAYER_RELAY_URL` | — | IP-layer Nostr relay for publishing birth IP Root event (kind 31900). |

## Minipae (NIP-AE Agent Identity)

| Variable | Default | Notes |
|---|---|---|
| `MINIPAE_URL` | — | Minipae service URL. |
| `MINIPAE_RELAY_URL` | — | Minipae Nostr relay. |
| `NIPAE_NSEC` | — | NIP-AE private key (nsec format). Derived from mnemonic at birth. |
| `NIPAE_RELAY_URL` | — | NIP-AE relay URL. |
| `NIPAE_BIRTH_KIND` | — | Nostr kind for birth announcements. |

## LLM / Inference

| Variable | Default | Notes |
|---|---|---|
| `AGENT_INFERENCE_URL` or `LARQL_URL` | — | Inference endpoint base URL. |
| `AGENT_INFERENCE_PROVIDER` | — | Provider name (e.g. `anthropic`, `openai`, `larql`). |
| `AGENT_INFERENCE_MODEL` | `mycelium-q4_k_m` | Model name. |
| `ANTHROPIC_API_KEY` | — | Anthropic API key (if using Claude). |
| `LARQL_URL` | — | LARQL server URL. |
| `LARQL_TOKEN` | — | LARQL auth token. |
| `LARQL_MODEL` | — | LARQL model override. |
| `LARQL_ENABLED` | — | Set `true` to enable LARQL routing. |

## GPU Compute

| Variable | Default | Notes |
|---|---|---|
| `GPUAI_API_KEY` or `GPU_AI_API_KEY` | — | GPU.ai API key for compute marketplace. |
| `KAGGLE_USERNAME` | — | Kaggle username for free GPU inference. |
| `KAGGLE_KEY` | — | Kaggle API key. |

## UCX (Universal Compute Exchange)

| Variable | Default | Notes |
|---|---|---|
| `UCX_BROKER_URL` | — | UCX broker URL. |
| `UCX_ALLOW_EXTERNAL` | — | Set `true` to allow external compute providers. |
| `UCX_DENY_PROVIDERS` | — | Comma-separated provider IDs to block. |
| `VANTAGE_UCX_COST_CAP_CENTS` | — | Max spend per session in USD cents. |
| `VANTAGE_UCX_MAX_GPU_HOURS` | — | Max GPU hours per session. |

## Sui / Blockchain

| Variable | Default | Notes |
|---|---|---|
| `OMOKODA_SUI_REGISTRY` | — | Sui registry object ID for agent registration. |
| `OMOKODA_ELEGBARA_PACKAGE` | — | Soul forge package ID on Sui. |
| `OMOKODA_ELEGBARA_ROUTER_ID` | — | Router object ID on Sui. |
| `BTC_BLOCK_HEIGHT` | — | Override BTC block height for Koodu time anchor. |
| `KOODU_BTC_API` | — | Bitcoin API URL for Koodu epoch sync. |

## VCP / DIP / Services

| Variable | Default | Notes |
|---|---|---|
| `VCP_BROKER_URL` | — | VCP device broker URL. |
| `DIP_URL` | — | DIP adapter URL. |
| `DIP_PORT` | `7792` | DIP service port. |
| `SOVEREIGN_NODE_URL` | — | Sovereign node URL for federation. |
| `OBATALA_URL` | — | Obatala service (knowledge/counsel). |
| `OGUN_URL` | — | Ogun service (execution/forge). |
| `SANGO_URL` | — | Ṣàngó receipt relay URL. |
| `WAGGLE_URL` | — | Waggle service URL. |
| `JULIA_OSUN_URL` | — | OSOVM Julia backend URL. |
| `OMOHOME_URL` | — | Omohome service URL. |

## Email (Agent Provisioning)

| Variable | Default | Notes |
|---|---|---|
| `AGENT_MAIL_DOMAIN` | — | Email domain for agent addresses. |
| `AGENT_MAIL_ADMIN_URL` | — | Mail admin API URL. |
| `AGENT_MAIL_ADMIN_TOKEN` | — | Admin token for mail provisioning. |

## SkillForge

| Variable | Default | Notes |
|---|---|---|
| `SKILLFORGE_HOME` | — | SkillForge skill directory. |
| `SKILLFORGE_FORGE` | — | Forge endpoint. |
| `SKILLFORGE_GATEWAY_PORT` | — | Gateway port. |
| `SKILLFORGE_VANTAGE_KEY` | — | Vantage key for skill publishing. |
| `SKILLFORGE_REQUIRE_FULL_AUDIT` | — | Set `true` to enforce full audit before install. |
| `SECURITY_REQUIRE_SIG` | — | Set `true` to require Ed25519 signatures on skill bundles. |

## Heartbeat / Daemon

| Variable | Default | Notes |
|---|---|---|
| `HEARTBEAT_SECS` | — | Heartbeat interval in seconds. |
| `JOB_DAEMON_POLL_SECS` | — | Job daemon polling interval. |
| `SKILL_DAEMON_SCAN_SECS` | — | Skill daemon scan interval. |

---

## Minimal Production Checklist

For a fully sovereign production node, set at minimum:

```bash
CHAIN_ID=mainnet
OMOKODA_ADMIN_TOKEN=<secret>
OMOKODA_HEARTBEAT_KEY=<base64url-32-byte-ed25519>
WALRUS_PUBLISHER_URL=https://publisher.walrus-testnet.walrus.space
WALRUS_AGGREGATOR_URL=https://aggregator.walrus-testnet.walrus.space
OMOKODA_TEE_SEAL=1
ZANGBETO_URL=http://localhost:7780
VANTAGE_URL=https://vantage.omokoda.com
VANTAGE_API_KEY=<agent-vantage-key>
NOSTR_RELAYS=wss://relay.damus.io,wss://nos.lol
```

Features that remain off without their env var are documented with `# fail-open` or `# gated` in their respective source files.
