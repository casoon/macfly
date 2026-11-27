# Changelog

All notable changes to MacFly are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.7.0] - 2026-11-27

### Breaking

- **CLI:** `macfly plan approve` applies the plan right after you type the code, so one command replaces approve plus apply; `--no-apply` only approves. Scripts that ran `plan apply` after `plan approve` now find the plan already applied.

### Added

- **CLI:** `macfly config init` asks for language and security level with arrow-key menus (and `--language en|de`); the language is stored as `language` in the config.
- **Config:** `export.allow_dirs` limits where plans may write files; by default only folders below the home folder, never `~/Library`.
- **MCP:** tools `photos_albums`, `photos_list`, `music_playlists`, `music_tracks` and `music_stats` (25 tools in total); exporting and playlist changes go through `plan_create`.
- **Music:** `macfly music playlists` and `macfly music tracks` read Apple Music playlists and tracks; `macfly music add` creates a plan (`music_playlist_add`, risk low) that creates a playlist if needed and adds library tracks by ID. Details: [Apple Music](https://casoon.github.io/macfly/docs/guide/music/).
- **Music:** `macfly music stats` shows listening statistics from the per-track counters (top tracks, artists, albums and genres, most skipped, rediscover, never played); `--csv` writes every track with its counters for spreadsheets. Details: [Listening statistics](https://casoon.github.io/macfly/docs/guide/music/#listening-statistics).
- **Photos:** `macfly photos albums` and `macfly photos list` read albums and photos (title, description, keywords, date, location) from Apple Photos; titles and descriptions pass through the DLP firewall.
- **Photos:** `macfly photos export` creates a plan (`photos_export`, risk medium, so it needs your approval under `balanced`) that exports originals or rendered JPEGs, optionally with an XMP or JSON metadata file per photo and a conversion with the built-in `sips`. Files are never overwritten, and a `.sha256` manifest lands next to them.

### Changed

- **CLI:** the approval view and the result of `plan approve`/`plan apply` are bilingual (English by default; set `"language": "de"` in the config, let `macfly config init` ask, or override with `MACFLY_LANG=de|en`) and show a numbered flow; serial operations are summarised, reviewable content such as mail drafts and note text stays in full, and `--details` adds every detail.
- **CLI:** the menus of `macfly config init` wrap long descriptions on narrow terminals instead of cutting them, and Music listings print as aligned tables (runemark 0.12).
- **Pages:** the project site uses the current shared theme (0.19.0).

## [0.6.0] - 2026-11-25

### Breaking

- **Plans:** under the new default preset `balanced`, low-risk plans (new notes, folders, mail drafts, reminders) are created as `approved` without `macfly plan approve`; `"preset": "strict"` keeps the old behaviour. Details: [Security Presets](https://casoon.github.io/macfly/docs/guide/plans-and-workflows/#security-presets).
- **CLI:** `macfly plan approve` no longer accepts `--confirm-code`; the code is typed at the interactive prompt.

### Added

- **Config:** security presets `strict`, `balanced` (default) and `open` per profile (`"preset"`), chosen once by `macfly config init` or `--preset`.
- **CLI:** `macfly policy` explains in plain language what the active profile may do; `macfly doctor` shows the preset and warns while `open` is active.
- **MCP:** `plan_apply` runs plans that are already approved (by the preset or by you); `plan_approve` stays unavailable over MCP.
- **DLP:** new `password` rule redacts the value after keywords like `Passwort:`, `Kennwort:` or `PIN=` (part of the default `redact` list; existing configs add `"password"` themselves).
- **Plans:** `approved_by` records whether a plan was approved by a preset (`preset:<name>`) or by a human.

### Changed

- **DLP:** `credit_card` also matches card numbers without separators (13–19 digits) and only redacts numbers with a valid Luhn checksum.
- **CLI:** `--no-redact` on `notes read/export` and `mail read/export` only works in an interactive terminal.
- **Skill:** the exported `SKILL.md` tells agents to apply auto-approved plans, hand pending ones to the user, and never fake a terminal or edit MacFly's config and plan files.

### Fixed

- **Mail:** `macfly mail list` and `macfly mail search` no longer time out on large mailboxes (e.g. an iCloud INBOX with ~40,000 messages); messages are now addressed by id and searches read subject and sender in one batch, so both finish in a few seconds.
- **Mail:** `macfly mail read` and `macfly mail export` look a message up by id instead of scanning every mailbox, so a single read takes about a second even in a large iCloud INBOX.
- **Mail:** the default `INBOX` filter also matches localized inboxes such as Exchange's `Posteingang`, so `mail list`, `mail search` and `mail read` no longer skip them.
- **Mail:** `macfly mail overview` reports the unread count of every mailbox instead of 0 for all but the first account's inbox.
- **Plans:** applying `mail_mark_read` and `mail_move` operations finds the message by id as well, so they no longer stall on large mailboxes.

### Security

- **Plans:** the approval code is random instead of derived from the plan hash, and it no longer appears in plan output, JSON, MCP responses or the error for a wrong code.
- **Releases:** macOS binaries carry signed build provenance; verify a download with `gh attestation verify` (see README).

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
