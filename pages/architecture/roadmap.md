---
title: Roadmap
description: Implemented capabilities and upcoming security & performance milestones.
order: 3
---

## Implemented (v0.1)

- **Pure Rust 3-crate workspace** (`macfly-core`, `macfly-adapters`, `macfly-cli`)
  with deterministic unit tests and zero Xcode/Swift dependencies
- **Single-Point-of-Truth config** (`~/.config/macfly/config.json`) with
  profiles, allowlists/denylists, DLP redaction, and Apple Mail signature lookup
- **Apple Notes:** live container hierarchy paths, cleanup audit, search,
  read with fidelity guard, lossless `notes_move` and `notes_move_folder_contents`,
  and safe `notes_delete_empty_folder`
- **Apple Mail:** overview, direct signature lookup, list, search, quote-trimmed
  read with `untrusted_external` framing, draft creation, mark-read, and move
- **Apple Calendar & Reminders:** upcoming events, open/overdue reminders, and
  unified 4-app `macfly context daily-brief`
- **Transactional plan engine:** `plan create → inspect → approve → apply` with
  SHA-256 tamper detection, interactive TTY guard, and JSONL audit journal
- **Built-in `macfly mcp` (`stdio`) server** exposing 11 read & plan-propose tools

## Next Milestones

- **Privilege Separation (`macfly-broker`):** Separate signed local macOS
  helper holding the TCC permissions over a restricted Unix socket, so AI
  agents can run with zero macOS Automation rights
- **Out-of-Band Touch ID approval (`LAContext`):** Native macOS approval sheet
  bound to the canonical SHA-256 plan hash for high-risk plans
- **Local SQLite FTS5 cache:** Incremental indexing for instant full-text search
  across 10,000+ email archives without touching private Apple databases by default
- **Batch JXA plan execution:** Run multi-operation plans inside a single
  script invocation for even faster large-scale reorganizations
