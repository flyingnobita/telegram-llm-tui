use telegram_llm_core::telegram::CachedMessage;
use time::OffsetDateTime;

pub fn format_transcript(messages: &[CachedMessage]) -> String {
    let mut transcript = String::new();
    for message in messages {
        let timestamp = OffsetDateTime::from_unix_timestamp(message.timestamp)
            .ok()
            .map(|dt| {
                format!(
                    "{}-{:02}-{:02} {:02}:{:02}",
                    dt.year(),
                    dt.month() as u8,
                    dt.day(),
                    dt.hour(),
                    dt.minute()
                )
            })
            .unwrap_or_else(|| message.timestamp.to_string());

        let author = message.author_name.as_deref().unwrap_or("Unknown");
        transcript.push_str(&format!(
            "[{}] {}: {}
",
            timestamp, author, message.text
        ));
    }
    transcript
}

#[cfg(test)]
mod tests {
    use super::*;
    use telegram_llm_core::telegram::{AuthorId, ChatId, MessageId, UserId};

    #[test]
    fn formats_transcript_correctly() {
        let messages = vec![
            CachedMessage {
                chat_id: ChatId(1),
                message_id: MessageId(1),
                author_id: AuthorId::User(UserId(1)),
                author_name: Some("Alice".to_string()),
                timestamp: 1672531200, // 2023-01-01 00:00:00 UTC
                edit_timestamp: None,
                text: "Hello there".to_string(),
                outgoing: false,
            },
            CachedMessage {
                chat_id: ChatId(1),
                message_id: MessageId(2),
                author_id: AuthorId::User(UserId(2)),
                author_name: None,
                timestamp: 1672531260, // 2023-01-01 00:01:00 UTC
                edit_timestamp: None,
                text: "General Kenobi".to_string(),
                outgoing: true,
            },
        ];

        let transcript = format_transcript(&messages);
        
        // Note: The timestamp formatting depends on the system's local offset if OffsetDateTime uses it,
        // but from_unix_timestamp returns UTC. Wait, OffsetDateTime::from_unix_timestamp returns a result
        // in UTC. So let's check the output.
        // Actually, the format_transcript uses `dt.year()` etc which works on OffsetDateTime.
        // Let's see if it produces UTC or if I need to be careful about timezones. 
        // `from_unix_timestamp` constructs a UTC datetime.
        
        assert!(transcript.contains("Alice: Hello there"));
        assert!(transcript.contains("Unknown: General Kenobi"));
        assert!(transcript.contains("2023-01-01 00:00"));
        assert!(transcript.contains("2023-01-01 00:01"));
    }
}