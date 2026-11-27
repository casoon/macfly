---
title: Roadmap
description: Implemented capabilities and upcoming security & performance milestones.
order: 3
---

## Implemented (v0.7.0)

- **Pure Rust 3-crate workspace** (`macfly-core`, `macfly-adapters`, `macfly`)
  published on `crates.io` with GitHub OIDC Trusted Publishing, plus a direct
  macOS binary installer (`install.sh`, SHA-256 verified, no Cargo required)
- **Single-Point-of-Truth config** (`~/.config/macfly/config.json`) with
  profiles, security presets (`strict` / `balanced` / `open`, explained by
  `macfly policy`), allowlists/denylists, DLP redaction (private keys, API keys,
  IBANs, Luhn-checked card numbers, OTP codes, passwords after keywords), and
  Apple Mail signature lookup
- **Apple Notes:** live container hierarchy paths, cleanup audit, search,
  read with fidelity guard, lossless `notes_move` and `notes_move_folder_contents`,
  safe `notes_delete_empty_folder`, Markdown export, and multi-format import
  (`.md`, `.txt`, `.html`, `.json`, `.jsonl`, `.csv` with `--recursive`)
- **Apple Mail:** overview with per-mailbox unread counts, direct signature
  lookup, list and search that scale with `--limit` instead of mailbox size,
  quote-trimmed read with `untrusted_external` framing, localized inbox names
  (e.g. `Posteingang`), draft creation, mark-read, move, and Markdown export
  (`mail export`)
- **Apple Contacts:** fast batched search, duplicate/empty-contact audit
  (`contacts audit`), and Markdown contact card export (`contacts export`)
- **Apple Calendar & Reminders:** upcoming events, open/overdue reminders,
  unified `daily-brief`, `person`, `meeting-prep`, and `weekly-review` context
  packs, direct PDF/Typst/Markdown reports (`macfly report`), Markdown export,
  and multi-format Reminders import (`PlanOperation::RemindersCreate`)
- **Apple Safari:** open tab inspection (`safari tabs`), Markdown export, and
  direct tab-to-note migration via plan (`safari to-note`)
- **Apple Photos:** albums and photo metadata (DLP-filtered), export of
  originals or rendered JPEGs via plan (`photos_export`) with XMP/JSON metadata,
  `sips` conversion and a SHA-256 manifest, limited by `export.allow_dirs`
- **Apple Music:** playlists and tracks, listening statistics from the play
  counters (`music stats`, CSV/JSON), and playlists built via plan
  (`music_playlist_add`)
- **Apple Shortcuts (`macfly shortcuts`):** allowlist-gated bridge to macOS
  `shortcuts list` / `shortcuts run` (`RiskLevel::High`, `Irreversible`)
- **Terminal presentation with `runemark`:** semantic verdicts, reports,
  error blocks, and `NO_COLOR` support
- **Hardened, resumable plan engine:** `plan create → inspect → approve → apply`
  with full-content diff previews, SHA-256 tamper detection, security presets
  (`strict` / `balanced` / `open`) with `macfly policy`, interactive approval
  with a random code, exclusive `.lock` files,
  `partially_applied` status with `--resume`, and JSONL audit journal
- **Built-in `macfly mcp` (`stdio`) server** exposing 25 read, context-pack,
  plan-propose and apply-approved tools with protocol negotiation (up to `2026-07-28`) and MCP tool
  annotations (`readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint`)
- **Agent Skill Export & `launchd` scheduler:** `macfly skill export` and
  `macfly schedule install` / `list` / `remove`

## Next Milestones

- **Privilege Separation (`macfly-broker`):** Separate signed local macOS
  helper holding the TCC permissions over a restricted Unix socket, so AI
  agents can run with zero macOS Automation rights
- **Out-of-Band Touch ID approval (`LAContext`):** Native macOS approval sheet
  bound to the canonical SHA-256 plan hash for high-risk plans
- **Focused Workspace Hygiene:** Deliberate, non-bloated audit & plan workflows
  for Downloads, Desktop, browser tabs, and stale build artifacts (always via
  macOS Trash, never blind `rm -rf`)
- **Local SQLite FTS5 cache:** Incremental indexing for instant full-text search
  across 10,000+ email archives without touching private Apple databases by default
