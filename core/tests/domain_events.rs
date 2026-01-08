use grammers_client::types::update::Raw;
use grammers_client::Update;
use grammers_session::updates::State;
use grammers_tl_types as tl;
use telegram_llm_core::telegram::{
    ChatId, DomainEvent, EventMapper, EventReceiver, MessageId, ReadReceipt, Typing, UserId,
};

fn state_with_date(date: i32) -> State {
    State {
        date,
        seq: 0,
        message_box: None,
    }
}

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

fn wrap_raw_update(update: tl::enums::Update, state: State) -> Update {
    Update::Raw(Raw { raw: update, state })
}

#[test]
fn maps_new_message_update() {
    let mapper = EventMapper::new();
    let message = base_message(1001, 1001, 42, 111, "hello");
    let update = tl::types::UpdateNewMessage {
        message: tl::enums::Message::Message(message),
        pts: 1,
        pts_count: 1,
    };
    let update = wrap_raw_update(tl::enums::Update::NewMessage(update), state_with_date(999));

    let event = mapper.map_update(&update).expect("expected domain event");
    match event {
        DomainEvent::MessageNew(payload) => {
            assert_eq!(payload.chat_id, ChatId(1001));
            assert_eq!(payload.message_id, MessageId(42));
            assert_eq!(payload.author_id, UserId(1001));
            assert_eq!(payload.timestamp, 111);
            assert_eq!(payload.text, "hello");
            assert!(!payload.outgoing);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn maps_edited_message_update() {
    let mapper = EventMapper::new();
    let mut message = base_message(1002, 1002, 7, 200, "edited");
    message.edit_date = Some(250);
    let update = tl::types::UpdateEditMessage {
        message: tl::enums::Message::Message(message),
        pts: 2,
        pts_count: 1,
    };
    let update = wrap_raw_update(tl::enums::Update::EditMessage(update), state_with_date(999));

    let event = mapper.map_update(&update).expect("expected domain event");
    match event {
        DomainEvent::MessageEdited(payload) => {
            assert_eq!(payload.chat_id, ChatId(1002));
            assert_eq!(payload.message_id, MessageId(7));
            assert_eq!(payload.editor_id, UserId(1002));
            assert_eq!(payload.timestamp, 250);
            assert_eq!(payload.text, "edited");
            assert!(!payload.outgoing);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn maps_read_receipt_update() {
    let mapper = EventMapper::new();
    let update = tl::types::UpdateReadHistoryOutbox {
        peer: peer_user(2001),
        max_id: 77,
        pts: 10,
        pts_count: 1,
    };
    let update = wrap_raw_update(
        tl::enums::Update::ReadHistoryOutbox(update),
        state_with_date(444),
    );

    let event = mapper.map_update(&update).expect("expected domain event");
    match event {
        DomainEvent::ReadReceipt(ReadReceipt {
            chat_id,
            reader_id,
            timestamp,
            last_read_message_id,
        }) => {
            assert_eq!(chat_id, ChatId(2001));
            assert_eq!(reader_id, UserId(2001));
            assert_eq!(timestamp, 444);
            assert_eq!(last_read_message_id, MessageId(77));
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn maps_typing_update() {
    let mapper = EventMapper::new();
    let update = tl::types::UpdateUserTyping {
        user_id: 3001,
        top_msg_id: None,
        action: tl::enums::SendMessageAction::SendMessageTypingAction,
    };
    let update = wrap_raw_update(tl::enums::Update::UserTyping(update), state_with_date(321));

    let event = mapper.map_update(&update).expect("expected domain event");
    match event {
        DomainEvent::Typing(Typing {
            chat_id,
            user_id,
            timestamp,
        }) => {
            assert_eq!(chat_id, ChatId(3001));
            assert_eq!(user_id, UserId(3001));
            assert_eq!(timestamp, 321);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn drops_oldest_when_buffer_full() {
    let (sender, receiver) = tokio::sync::broadcast::channel(2);
    let mut receiver = EventReceiver::from_receiver(receiver);

    let first = DomainEvent::Typing(Typing {
        chat_id: ChatId(1),
        user_id: UserId(1),
        timestamp: 1,
    });
    let second = DomainEvent::Typing(Typing {
        chat_id: ChatId(2),
        user_id: UserId(2),
        timestamp: 2,
    });
    let third = DomainEvent::Typing(Typing {
        chat_id: ChatId(3),
        user_id: UserId(3),
        timestamp: 3,
    });

    let _ = sender.send(first);
    let _ = sender.send(second);
    let _ = sender.send(third);

    match receiver.recv().await {
        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
        other => panic!("expected lagged receiver error, got: {other:?}"),
    }
}

#[tokio::test]
async fn warns_on_lagged_subscriber() {
    let (sender, receiver) = tokio::sync::broadcast::channel(1);
    let mut receiver = EventReceiver::from_receiver(receiver);

    let first = DomainEvent::Typing(Typing {
        chat_id: ChatId(10),
        user_id: UserId(10),
        timestamp: 10,
    });
    let second = DomainEvent::Typing(Typing {
        chat_id: ChatId(11),
        user_id: UserId(11),
        timestamp: 11,
    });

    let _ = sender.send(first);
    let _ = sender.send(second);

    match receiver.recv().await {
        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
        other => panic!("expected lagged receiver error, got: {other:?}"),
    }

    let next = receiver.recv().await.expect("expected next event");
    match next {
        DomainEvent::Typing(payload) => {
            assert_eq!(payload.chat_id, ChatId(11));
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn ignores_unsupported_updates() {
    let mapper = EventMapper::new();
    let update = wrap_raw_update(tl::enums::Update::Config, state_with_date(1));

    assert!(mapper.map_update(&update).is_none());
}
