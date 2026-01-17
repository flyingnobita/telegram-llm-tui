# Core Code Review

## 2026-01-17

1. High - `parse_message` drops any message whose author peer is not a
   `Peer::User`, so channel posts, anonymous admin posts, and messages missing
   `from_id` never surface in events/history/cache. Consider supporting channel
   sender identities or allowing an optional author for these cases.
   `core/src/telegram/events.rs:293`
2. Medium - Typing events are only mapped for `Update::UserTyping`. Group and
   channel typing updates (`UpdateChatUserTyping`, `UpdateChannelUserTyping`)
   are treated as unsupported and dropped. `core/src/telegram/events.rs:81`
3. Medium - `SendPipeline::stop` breaks the worker loop without draining the
   queue, leaving queued items with no terminal status update. Callers can hang
   waiting on `SendStatus`. Consider marking in-flight items as failed on stop.
   `core/src/telegram/send.rs:270`
4. Low - When retry attempts are exhausted, the failure is marked
   `retryable: true`, even though no more retries will happen. This is
   misleading for consumers. `core/src/telegram/send.rs:402`
5. Low - `fetch_recent_messages` applies the limit before `parse_message`, so
   returned results can be fewer than `limit` if messages are filtered out. If
   callers expect exactly `limit`, consider over-fetch or documenting.
   `core/src/telegram/history.rs:16`
