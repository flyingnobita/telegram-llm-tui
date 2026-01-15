use grammers_client::Client;
use grammers_session::defs::{PeerId, PeerKind};
use grammers_tl_types as tl;

use crate::telegram::cache::{ChatPeerKind, ChatSummary};
use crate::telegram::error::Result;
use crate::telegram::events::{ChatId, MessageId};

pub async fn fetch_dialog_summaries(client: &Client) -> Result<Vec<ChatSummary>> {
    let mut dialogs = client.iter_dialogs();
    let mut summaries = Vec::new();

    while let Some(dialog) = dialogs.next().await? {
        if matches!(dialog.raw, tl::enums::Dialog::Folder(_)) {
            continue;
        }

        let peer = dialog.peer();
        let peer_id = peer.id();
        let chat_id = ChatId(peer_id.bot_api_dialog_id());
        let title = resolve_chat_title(peer.name(), peer.username(), chat_id);
        let peer_kind = peer_kind_from_peer_id(peer_id);

        let (last_message_id, last_message_at) = dialog
            .last_message
            .as_ref()
            .map(|message| {
                (
                    Some(MessageId(message.id() as i64)),
                    Some(message.date().timestamp()),
                )
            })
            .unwrap_or((None, None));

        let unread_count = match &dialog.raw {
            tl::enums::Dialog::Dialog(raw) => Some(raw.unread_count.max(0) as u32),
            tl::enums::Dialog::Folder(_) => None,
        };

        summaries.push(ChatSummary {
            chat_id,
            title,
            peer_kind,
            last_message_id,
            last_message_at,
            unread_count,
        });
    }

    Ok(summaries)
}

fn resolve_chat_title(name: Option<&str>, username: Option<&str>, chat_id: ChatId) -> String {
    let trimmed_name = name.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    if let Some(trimmed_name) = trimmed_name {
        return trimmed_name.to_string();
    }

    let trimmed_username = username.and_then(|value| {
        let trimmed = value.trim().trim_start_matches('@');
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    if let Some(trimmed_username) = trimmed_username {
        return format!("@{}", trimmed_username);
    }

    format!("Chat {}", chat_id.0)
}

fn peer_kind_from_peer_id(peer_id: PeerId) -> ChatPeerKind {
    match peer_id.kind() {
        PeerKind::User | PeerKind::UserSelf => ChatPeerKind::User,
        PeerKind::Chat => ChatPeerKind::Group,
        PeerKind::Channel => ChatPeerKind::Channel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_chat_title_prefers_name() {
        let chat_id = ChatId(100);
        let title = resolve_chat_title(Some("Team Chat"), Some("teamchat"), chat_id);
        assert_eq!(title, "Team Chat");
    }

    #[test]
    fn resolve_chat_title_uses_username_when_name_missing() {
        let chat_id = ChatId(200);
        let title = resolve_chat_title(None, Some("teamchat"), chat_id);
        assert_eq!(title, "@teamchat");
    }

    #[test]
    fn resolve_chat_title_falls_back_to_id() {
        let chat_id = ChatId(-100123);
        let title = resolve_chat_title(Some("  "), Some(""), chat_id);
        assert_eq!(title, "Chat -100123");
    }

    #[test]
    fn peer_kind_from_peer_id_maps_correctly() {
        assert_eq!(peer_kind_from_peer_id(PeerId::user(1)), ChatPeerKind::User);
        assert_eq!(peer_kind_from_peer_id(PeerId::chat(2)), ChatPeerKind::Group);
        assert_eq!(
            peer_kind_from_peer_id(PeerId::channel(3)),
            ChatPeerKind::Channel
        );
    }
}
