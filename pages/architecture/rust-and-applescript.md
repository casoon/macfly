---
title: Why Rust + AppleScript/JXA
description: Why pairing a deterministic Rust core with a minimal AppleScript/JXA boundary pays off for long-term maintenance, testing, and security.
order: 1
---

When building macOS automation for AI agents, two extremes dominate: heavyweight
scripting wrappers with no real type or security layer, or deeply coupled
platform-specific apps tied to Xcode projects, private frameworks, and fragile
internal SQLite schemas.

MacFly deliberately chooses a third path: **a pure, deterministic Rust core
paired with a thin, static AppleScript/JXA execution boundary.**

## 1. Determinism and fast offline testing

Over 90 % of what makes an automation layer trustworthy has nothing to do with
macOS GUI APIs:

- Profile and scope policy evaluation (`allow_accounts`, `deny_folders`,
  `deny_mailboxes`)
- Read-side DLP redaction (linear-time Rust `regex`, immune to ReDoS)
- Parsing untrusted email bodies and Apple Notes HTML without memory-safety bugs
- Note fidelity analysis (detecting attachments, tables, checklists, internal
  links before allowing edits)
- Canonical SHA-256 plan hashing, tamper detection, TTL checks, and audit
  journaling

Keeping all of this in `macfly-core` means the entire security and policy engine
is **100 % deterministic** and verified in milliseconds with `cargo test` — even
without Apple Notes or Apple Mail running.

## 2. Long-term maintainability with zero SDK drift

- **No Xcode project, no Swift toolchain lock-in, no Node.js runtime:** A
  single `Cargo.toml` workspace compiles to one fast native binary.
- **No brittle private database assumptions in the default path:** Apple Notes
  and Apple Mail do not expose public native SDKs for external automation;
  internal SQLite schemas (`NoteStore.sqlite`, `Envelope Index`) change across
  macOS releases and require Full Disk Access. Using the official Open Scripting
  Architecture (`osascript -l JavaScript`) avoids Full Disk Access by default
  and keeps macOS upgrades low-friction.
- **Swappable adapter boundary:** Every app lives behind a clean Rust adapter
  module. If a future domain benefits from a native binding or an optional
  read-only local index, only that adapter changes — the CLI, MCP server, policy
  engine, and plan contract stay untouched.

## 3. Making AppleScript/JXA safe and fast

AppleScript has a bad reputation mainly because tools invoke it naively:
concatenating strings into shell commands and firing one Apple Event per item.
MacFly enforces four strict engineering rules in its JXA runner:

1. **Static scripts only (`include_str!`):** User input is never interpolated
   into script source code, eliminating script injection by construction.
2. **JSON over `stdin`, never `argv`:** Passing search queries or note contents
   as CLI arguments briefly exposes them to other local processes via `ps aux`.
   MacFly pipes JSON over `stdin` and parses it via Foundation's
   `NSFileHandle.fileHandleWithStandardInput`.
3. **Batched property fetches:** Instead of sending `N × 5` Apple Events for
   `N` notes, MacFly fetches folder properties in bulk (`folder.notes.id()`,
   `folder.notes.name()`, `folder.notes.modificationDate()`) — turning minute-long
   scans into a few seconds.
4. **Hard timeouts and typed TCC errors:** Every `osascript` call runs under a
   configurable timeout with clean process termination, and macOS `-1743`
   permission denials are mapped directly to structured `TCC_PERMISSION_DENIED`
   errors with actionable hints.
