---
title: Overview
description: What MacFly is, why it pairs Rust with AppleScript/JXA, and how the documentation is organized.
order: 1
---

**MacFly** is a local, model-independent Rust CLI and `stdio` MCP server that
puts a **deterministic execution layer and security firewall** between AI
agents (or shell workflows) and your personal macOS data:

- **Apple Notes:** hierarchical folder overview, cleanup audit, search,
  read with fidelity guard, and lossless batch moves.
- **Apple Mail:** account & mailbox overview, direct signature lookup from
  Apple Mail, search, quote-trimmed reading, and safe draft creation.
- **Apple Calendar & Reminders:** upcoming events, open/overdue reminders,
  and a unified `daily-brief` context pack across all four apps.
- **Apple Contacts & Safari:** contact search and audit, open tabs to Markdown or a note.
- **Apple Photos:** albums and photo metadata, export of originals or rendered
  JPEGs with XMP/JSON metadata through a reviewed plan.
- **Apple Music:** playlists, listening statistics from the play counters, and
  playlists built through a reviewed plan.
- **Apple Shortcuts:** allowlist-gated runs for Focus modes, HomeKit scenes
  and App Intents.

## Core design principles

1. **Deterministic Rust core, minimal macOS boundary:** All security, policy,
   DLP redaction, HTML normalization, hashing, and journaling logic is pure
   Rust. macOS automation happens through static, embedded JXA scripts
   receiving JSON over `stdin`.
2. **Security on both sides (Read & Write):** Most automation tools only
   guard writes (if at all). MacFly protects *reads* via profile scopes and
   automatic DLP redaction, and *writes* via tamper-evident SHA-256 plans and
   a security preset (`strict` / `balanced` / `open`) that decides which
   low-risk changes run automatically and which need your approval.
3. **Single Point of Truth:** One central `~/.config/macfly/config.json` for
   scopes and DLP rules. Email accounts stay in Apple Mail, and email
   signatures are read straight from Apple Mail instead of being duplicated.
4. **Honest guarantees:** Operations declare their risk level (`low`, `medium`,
   `high`) and reversibility (`reversible`, `best_effort`, `irreversible`), and
   rich Apple Notes are explicitly protected against destructive edits.

## Where to go next

- **[Getting Started](/macfly/docs/guide/getting-started/):** Build,
  permissions, config schema, and everyday CLI commands.
- **[Plans & Workflows](/macfly/docs/guide/plans-and-workflows/):** Full
  reference for `Plan → Inspect → Approve → Apply` and all supported mutations.
- **[Migrations & Backups](/macfly/docs/guide/migrations/):** Two-way
  pipelines between Apple apps and Markdown, CSV, JSON, JSONL, and HTML.
- **[Cleaning Up Apple Notes](/macfly/docs/guide/cleaning-up-notes/):**
  Structure-agnostic 4-phase field guide & real-world Apple Notes edge cases.
- **[Apple Shortcuts & Automation](/macfly/docs/guide/shortcuts/):**
  Allowlist-gated `shortcuts run` for Focus modes, HomeKit scenes, and App Intents.
- **[Apple Music](/macfly/docs/guide/music/):**
  Playlists, listening statistics and playlist plans.
- **[MCP & AI Agents](/macfly/docs/guide/mcp-and-agents/):** Connecting
  Claude Desktop, Cursor, Codex, or custom agents safely.
- **[Why Rust + AppleScript/JXA](/macfly/docs/architecture/rust-and-applescript/):**
  Architectural rationale for determinism, maintainability, and zero SDK drift.
- **[Security Model](/macfly/docs/architecture/security-model/):** Full threat
  model, DLP rules, prompt-injection defense, and trust boundaries.
