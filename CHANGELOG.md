# Changelog

All notable changes to MacFly are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Fixed
- **Mail:** `macfly mail list` and `macfly mail search` no longer time out on large mailboxes (e.g. an iCloud INBOX with ~40,000 messages); messages are now addressed by id and searches read subject and sender in one batch, so both finish in a few seconds.
- **Mail:** `macfly mail overview` reports the unread count of every mailbox instead of 0 for all but the first account's inbox.

## [0.5.0] - 2026-11-22

### Added
- **Direct macOS binary installer (`install.sh`)** — install prebuilt Apple Silicon and Intel binaries from GitHub Releases with SHA-256 verification and no Cargo/Rust toolchain required.
- **Full-content plan preview & diffs** in `macfly plan inspect` and `macfly plan approve`, showing complete Apple Mail draft bodies, line-by-line Apple Notes additions, and explicit high-risk warnings for Apple Shortcuts.
- **Resumable execution & concurrency lock** — exclusive `.lock` guard against duplicate runs, `partially_applied` status on mid-plan errors, per-operation progress persistence, and `macfly plan apply <id> --resume`.
- **MCP protocol negotiation & tool annotations** — negotiates up to MCP `2026-07-28` (with backwards compatibility to `2024-11-05`) and annotates all 19 tools with `readOnlyHint`, `destructiveHint`, `idempotentHint`, and `openWorldHint`.

### Changed
- **Strict TTY approval guard:** `macfly plan approve` now requires an interactive terminal even when `--confirm-code` is passed, closing non-interactive self-approval paths.
- **Shortcut risk classification:** `shortcuts.run` operations are now classified as `high` risk and `irreversible`.

## [0.4.0] - 2026-11-22

### Added
- Cross-app **Context Packs** (`macfly context daily-brief`, `person`, `meeting-prep`, `weekly-review`) correlating Apple Notes, Mail, Contacts, Calendar, and Reminders in one command and over MCP.
- **Typst & Markdown Dossier Export** (`macfly report person`, `daily-brief`, `weekly-review`) ready for `typst compile` and `renderreport`.
- **Agent Skill Export** (`macfly skill export`) generating a ready-to-use `SKILL.md` for Claude Code, Codex, Cursor, and Gemini CLI.
- **Local `launchd` Schedules** (`macfly schedule add`, `list`, `remove`) for recurring macOS routines without external servers.

### Fixed
- Eliminated a blocking JXA `signature.content()` call in `mail_overview.js`, reducing `macfly mail overview` from >25 s to ~0.5 s and `macfly mail signatures` to ~0.24 s.

## [0.3.0] - 2026-11-22

### Added
- **Multi-format two-way migrations** for Apple Notes and Reminders supporting `.md`, `.txt`, `.html`, `.json`, `.jsonl`, and RFC-4180 `.csv` (Obsidian, Notion, Evernote, Todoist).
- **Contacts, Calendar, Reminders & Safari adapters** including `contacts audit`, `contacts export`, `calendar export`, `reminders export/import`, and `safari tabs` / `safari capture`.
- **Apple Mail export** (`macfly mail export`) writing clean Markdown archives with YAML frontmatter and thread metadata.

## [0.2.0] - 2026-11-22

### Added
- **Apple Shortcuts adapter** (`macfly shortcuts list`, `run`) guarded by an explicit `shortcuts.allow` profile allowlist.
- **Note Fidelity Guard** protecting Apple Notes containing attachments, tables, checklists, or `applenotes://` links during automated reorganizations.
- **Notes hygiene audit** (`macfly notes audit`) identifying empty folders, single-note folders, and duplicate titles.

## [0.1.0] - 2026-11-22

### Added
- Pure Rust core (`macfly-core`, `macfly-adapters`, `macfly`) with static, `stdin`-only JXA/AppleScript edge.
- Read-side **Data Loss Prevention (DLP)** firewall redacting private keys, API keys, IBANs, credit cards, and OTP codes.
- Transactional **`Plan → Inspect → Approve → Apply`** engine with canonical SHA-256 plan hashing, interactive TTY approval, and JSONL audit journal.
- Built-in **`stdio` MCP server** (`macfly mcp`) exposing read and `plan_create`/`plan_inspect` tools only.
- Terminal UI powered by `runemark`.
