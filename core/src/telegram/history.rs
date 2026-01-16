use grammers_client::Client;
use grammers_session::defs::PeerRef;

use crate::telegram::error::Result;
use crate::telegram::events::{parse_message, MessageNew};

pub async fn fetch_recent_messages(
    client: &Client,
    peer: PeerRef,
    limit: usize,
) -> Result<Vec<MessageNew>> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    let mut iter = client.iter_messages(peer).limit(limit);
    let mut raw_messages = Vec::new();

    while let Some(message) = iter.next().await? {
        raw_messages.push(message.raw);
    }

    Ok(map_raw_messages(raw_messages))
}

fn map_raw_messages(raw_messages: Vec<grammers_tl_types::enums::Message>) -> Vec<MessageNew> {
    let mut mapped = Vec::new();
    for raw in raw_messages.into_iter().rev() {
        if let Some(parsed) = parse_message(&raw) {
            mapped.push(MessageNew {
                chat_id: parsed.chat_id,
                message_id: parsed.message_id,
                author_id: parsed.author_id,
                timestamp: parsed.date,
                text: parsed.text,
                outgoing: parsed.outgoing,
            });
        }
    }
    mapped
}

#[cfg(test)]
mod tests {
    use super::*;
    use grammers_tl_types as tl;

    use crate::telegram::events::{ChatId, MessageId, UserId};

    fn peer_user(user_id: i64) -> tl::enums::Peer {
        tl::enums::Peer::User(tl::types::PeerUser { user_id })
    }

    fn base_message(
        user_id: i64,
        chat_user_id: i64,
        message_id: i32,
        date: i32,
        text: &str,
    ) -> tl::types::Message {
        tl::types::Message {
            out: false,
            mentioned: false,
            media_unread: false,
            silent: false,
            post: false,
            from_scheduled: false,
            legacy: false,
            edit_hide: false,
            pinned: false,
            noforwards: false,
            invert_media: false,
            offline: false,
            video_processing_pending: false,
            paid_suggested_post_stars: false,
            paid_suggested_post_ton: false,
            id: message_id,
            from_id: Some(peer_user(user_id)),
            from_boosts_applied: None,
            peer_id: peer_user(chat_user_id),
            saved_peer_id: None,
            fwd_from: None,
            via_bot_id: None,
            via_business_bot_id: None,
            reply_to: None,
            date,
            message: text.to_string(),
            media: None,
            reply_markup: None,
            entities: None,
            views: None,
            forwards: None,
            replies: None,
            edit_date: None,
            post_author: None,
            grouped_id: None,
            reactions: None,
            restriction_reason: None,
            ttl_period: None,
            quick_reply_shortcut_id: None,
            effect: None,
            factcheck: None,
            report_delivery_until_date: None,
            paid_message_stars: None,
            suggested_post: None,
        }
    }

    #[test]
    fn reverses_recent_messages_to_oldest_first() {
        let oldest = tl::enums::Message::Message(base_message(1, 1, 10, 100, "old"));
        let newest = tl::enums::Message::Message(base_message(1, 1, 11, 200, "new"));
        let mapped = map_raw_messages(vec![newest, oldest]);

        assert_eq!(mapped.len(), 2);
        assert_eq!(mapped[0].chat_id, ChatId(1));
        assert_eq!(mapped[0].message_id, MessageId(10));
        assert_eq!(mapped[0].author_id, UserId(1));
        assert_eq!(mapped[1].message_id, MessageId(11));
    }
}
