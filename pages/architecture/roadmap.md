---
title: Roadmap
description: Implemented capabilities and upcoming security & performance milestones.
order: 3
---

## Implemented (v0.3)

- **Pure Rust 3-crate workspace** (`macfly-core`, `macfly-adapters`, `macfly`)
  published on `crates.io` with GitHub OIDC Trusted Publishing
- **Single-Point-of-Truth config** (`~/.config/macfly/config.json`) with
  profiles, allowlists/denylists, DLP redaction, and Apple Mail signature lookup
- **Apple Notes:** live container hierarchy paths, cleanup audit, search,
  read with fidelity guard, lossless `notes_move` and `notes_move_folder_contents`,
  safe `notes_delete_empty_folder`, Markdown export, and multi-format import
  (`.md`, `.txt`, `.html`, `.json`, `.jsonl`, `.csv` with `--recursive`)
- **Apple Mail:** overview, direct signature lookup, list, search, quote-trimmed
  read with `untrusted_external` framing, draft creation, mark-read, move, and
  Markdown export (`mail export`)
- **Apple Contacts:** fast batched search, duplicate/empty-contact audit
  (`contacts audit`), and Markdown contact card export (`contacts export`)
- **Apple Calendar & Reminders:** upcoming events, open/overdue reminders,
  unified 4-app `macfly context daily-brief`, Markdown export, and multi-format
  Reminders import (`PlanOperation::RemindersCreate`)
- **Apple Safari:** open tab inspection (`safari tabs`), Markdown export, and
  direct tab-to-note migration via plan (`safari to-note`)
- **Apple Shortcuts (`macfly shortcuts`):** allowlist-gated bridge to macOS
  `shortcuts list` / `shortcuts run` via `PlanOperation::ShortcutsRun`
- **Terminal presentation with `runemark`:** semantic verdicts, reports,
  error blocks, and `NO_COLOR` support
- **Transactional plan engine:** `plan create → inspect → approve → apply` with
  SHA-256 tamper detection, interactive TTY guard, and JSONL audit journal
- **Built-in `macfly mcp` (`stdio`) server** exposing 15 read & plan-propose tools

## Next Milestones

- **Privilege Separation (`macfly-broker`):** Separate signed local macOS
  helper holding the TCC permissions over a restricted Unix socket, so AI
  agents can run with zero macOS Automation rights
- **Out-of-Band Touch ID approval (`LAContext`):** Native macOS approval sheet
  bound to the canonical SHA-256 plan hash for high-risk plans
- **Focused Workspace Hygiene:** Deliberate, non-bloated audit & plan workflows
  for Downloads, Desktop, browser tabs, and stale build artifacts (always via
  macOS Trash, never blind `rm -rf`)
- **Targeted App Adapters (only where stable `sdef` dictionaries exist):**
  Apple Photos (albums & targeted exports), Safari tabs (research capture into
  Notes), and Apple Contacts
- **Local SQLite FTS5 cache:** Incremental indexing for instant full-text search
  across 10,000+ email archives without touching private Apple databases by default
