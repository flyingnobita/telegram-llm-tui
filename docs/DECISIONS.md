# Index of Decisions

- Telegram client library: grammers → `docs/adr/20251231-telegram-client-grammers.md`
- TUI framework: ratatui → `docs/adr/20251231-tui-ratatui.md`
- LLM providers & auth strategy: OpenAI + local `.env` for dev-only (prod secret
  store later) → `docs/adr/20251231-llm-backend-openai-env.md`
- Data directory layout: local project dir for dev-only → `docs/adr/20251231-data-dir-local.md`
- LLM-friendly test framework: `insta` snapshots + `ratatui` test backend →
  `docs/adr/20260105-llm-friendly-test-framework.md`
- Logging policy (plain logs, 1 MB rotation, 20 files, content logging on) →
  `docs/adr/20260106-logging-policy.md`
- Cache persistence store: sqlite → `docs/adr/20260109-cache-store-sqlite.md`
