//! Buzz relay client -- connects this agent's derived Nostr keypair
//! (see buzz.rs) to a live Buzz relay over WebSocket and does a real
//! NIP-29 (relay-based groups) round trip: join the shared open channel,
//! read what's already posted there, and post a signed reply. NIP-42 relay
//! auth (if the relay challenges us) is handled automatically by
//! nostr-sdk's Client, since it already holds our signer.

use nostr_sdk::prelude::*;
use std::time::Duration;

/// Self-join a NIP-29 open group without posting a chat message -- for
/// /buzz-register, where the point is just to become a real member the
/// relay/buzz-acp will recognize, not to say anything yet.
pub async fn self_join(relay_url: &str, keys: Keys, group_id: &str) -> Result<(), String> {
    let client = Client::new(keys.clone());
    client
        .add_relay(relay_url)
        .await
        .map_err(|e| format!("add_relay failed: {e}"))?;
    client.connect().await;

    let join_event = EventBuilder::new(Kind::Custom(9021), "")
        .tag(Tag::custom(
            TagKind::SingleLetter(SingleLetterTag::lowercase(Alphabet::H)),
            [group_id],
        ))
        .sign(&keys)
        .await
        .map_err(|e| format!("join event signing failed: {e}"))?;
    client
        .send_event(&join_event)
        .await
        .map_err(|e| format!("join send_event failed: {e}"))?;

    client.disconnect().await;
    Ok(())
}

/// Join a NIP-29 open group (`group_id`, an `#h` tag value) on `relay_url`
/// and post one chat message into it, then listen briefly for any other
/// kind:9 messages already in (or arriving in) that group -- proof this
/// agent's identity is really talking to another identity through the
/// live relay, not just publishing into the void.
pub async fn join_and_chat(
    relay_url: &str,
    keys: Keys,
    group_id: &str,
    message: &str,
    listen_secs: u64,
) -> Result<(EventId, Vec<Event>), String> {
    join_and_chat_mentioning(relay_url, keys, group_id, message, None, listen_secs).await
}

/// Same as `join_and_chat`, but if `mention_pubkey_hex` is set, tags the
/// chat event with a real NIP-01 `p` tag -- the thing buzz-acp's mention
/// subscription actually filters on (a literal "@name" in the text is not
/// enough to trigger a mention-based agent).
pub async fn join_and_chat_mentioning(
    relay_url: &str,
    keys: Keys,
    group_id: &str,
    message: &str,
    mention_pubkey_hex: Option<&str>,
    listen_secs: u64,
) -> Result<(EventId, Vec<Event>), String> {
    let client = Client::new(keys.clone());
    client
        .add_relay(relay_url)
        .await
        .map_err(|e| format!("add_relay failed: {e}"))?;
    client.connect().await;

    // Read whatever's already in the group (Vantage's test message lands
    // here) before we post anything ourselves.
    let history_filter = Filter::new()
        .kind(Kind::Custom(9))
        .custom_tag(SingleLetterTag::lowercase(Alphabet::H), group_id);

    let history = client
        .fetch_events(history_filter.clone(), Duration::from_secs(10))
        .await
        .map_err(|e| format!("fetch_events failed: {e}"))?;
    let mut seen: Vec<Event> = history.into_iter().collect();

    // NIP-29 self-join: kind 9021, tag #h = group_id.
    let join_event = EventBuilder::new(Kind::Custom(9021), "")
        .tag(Tag::custom(
            TagKind::SingleLetter(SingleLetterTag::lowercase(Alphabet::H)),
            [group_id],
        ))
        .sign(&keys)
        .await
        .map_err(|e| format!("join event signing failed: {e}"))?;
    client
        .send_event(&join_event)
        .await
        .map_err(|e| format!("join send_event failed: {e}"))?;

    // Live subscription so we catch anything posted concurrently.
    client
        .subscribe(
            history_filter.since(Timestamp::now() - Duration::from_secs(5)),
            None,
        )
        .await
        .map_err(|e| format!("subscribe failed: {e}"))?;

    // Post our own kind:9 chat message into the group.
    let mut builder = EventBuilder::new(Kind::Custom(9), message).tag(Tag::custom(
        TagKind::SingleLetter(SingleLetterTag::lowercase(Alphabet::H)),
        [group_id],
    ));
    if let Some(pk_hex) = mention_pubkey_hex {
        let mentioned =
            PublicKey::from_hex(pk_hex).map_err(|e| format!("bad mention pubkey: {e}"))?;
        builder = builder.tag(Tag::public_key(mentioned));
    }
    let chat_event = builder
        .sign(&keys)
        .await
        .map_err(|e| format!("chat event signing failed: {e}"))?;
    let output = client
        .send_event(&chat_event)
        .await
        .map_err(|e| format!("chat send_event failed: {e}"))?;
    let my_id = *output.id();

    let mut notifications = client.notifications();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(listen_secs);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, notifications.recv()).await {
            Ok(Ok(RelayPoolNotification::Event { event, .. })) => {
                if event.id != my_id && event.kind == Kind::Custom(9) {
                    seen.push((*event).clone());
                }
            }
            Ok(Ok(_)) => continue,
            _ => break,
        }
    }

    client.disconnect().await;
    seen.sort_by_key(|e| e.created_at);
    Ok((my_id, seen))
}
