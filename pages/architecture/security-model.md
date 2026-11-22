---
title: Security Model
description: Full threat model, read-side DLP firewall, indirect prompt-injection defense, tamper-evident plans, and explicit system boundaries.
order: 2
---

Connecting AI agents to personal emails and notes creates risks on **both**
sides of the boundary: unintended mutations when writing, and silent data
exfiltration when reading. MacFly addresses both systematically.

## Threat Model

| ID | Threat | Attack Vector | MacFly Mitigation |
|---|---|---|---|
| **T1** | Indirect prompt injection (Write) | Malicious email instructs an agent to forward messages, delete notes, or alter data | Agents can only submit *proposals* (`plan_create`). Approval and execution are blocked from MCP and require interactive TTY confirmation. |
| **T2** | Read-side data exfiltration | Broad search sends passwords, IBANs, 2FA codes, or private folders to a cloud LLM | Profile-level `allow_accounts` / `deny_folders` / `deny_mailboxes` plus automatic regex DLP redaction (`[REDACTED:IBAN_1]`, etc.) before output. |
| **T3** | Self-approval via shell pipe | Agent pipes confirmation into `approve` | `macfly plan approve` enforces `isatty()` when no explicit hash code is given, and MCP does not expose `approve` or `apply` at all. |
| **T4** | Plan tampering & TOCTOU | Stored plan is edited between review and apply, or a note changed meanwhile | Plans are stored at mode `0600` with a canonical SHA-256 hash and expiry TTL. `load()` re-verifies the hash; `notes_append` / `notes_move` verify `expected_revision` pre-flight. |
| **T5** | Script & argument injection | Special characters break out of AppleScript, or secrets leak in `ps aux` | Statically embedded JXA scripts (`include_str!`); all inputs passed strictly as JSON via `stdin`. |
| **T6** | Silent data loss on rich notes | Setting HTML body overwrites attachments, tables, checklists, or `applenotes://` links | `analyze_note_fidelity` inspects every note and blocks `replace_body` when rich content is present; moves operate on native note objects losslessly. |
| **T7** | Unchecked irreversible actions | Sending an email or deleting a non-empty folder cannot be undone | Mail mutations create **drafts** for review (no blind send); `notes_delete_empty_folder` re-verifies zero notes and zero active subfolders live before deleting. |

## Read-Side Data Firewall (DLP)

Every `notes read` and `mail read` passes through the deterministic Rust DLP
pipeline before leaving the process:

1. **Scope enforcement:** Account, folder, and mailbox policies are checked
   before and after retrieval. Password-protected Apple Notes are skipped by
   default (`skip_password_protected: true`).
2. **Pattern redaction:** Built-in linear-time patterns mask:
   - `private_key` (PEM private key blocks)
   - `api_key` (`sk-...`, `ghp_...`, `AKIA...`, `AIza...`, `xox...`)
   - `iban` (European IBAN formats)
   - `credit_card` (16-digit card numbers)
   - `otp_code` (context-bound 6-digit verification / TAN / 2FA codes)
3. **Untrusted-data framing:** Incoming emails are explicitly tagged with
   `"trust_level": "untrusted_external"` and a `content_warning` reminding the
   consuming agent to treat the body strictly as data, never as instructions.
4. **Body length cap:** `max_body_chars` prevents runaway token consumption and
   accidental bulk dumps.

## Audit Journal

Every `plan apply` attempt (whether succeeded or failed) appends a structured
JSONL record to `~/.local/share/macfly/journal.jsonl` (permissions `0600`)
containing timestamp, `plan_id`, `profile`, `plan_hash`, status, and per-action
results. Inspect recent activity at any time with `macfly history`.

## Honest System Boundary

MacFly's CLI and MCP layer reliably protect every action routed through MacFly.
However, if an AI agent runs in the **same macOS user session** with
unrestricted shell access *and* its own macOS Automation (TCC) permissions, a
compromised agent could invoke `osascript` directly outside MacFly.

True OS-level enforcement requires privilege separation: running the agent
without TCC Automation permissions and routing requests over a local Unix socket
to a separately signed **`macfly-broker`** helper holding the TCC grant and
prompting via macOS Touch ID (`LAContext`). This is the next major milestone on
the [Roadmap](/macfly/docs/architecture/roadmap/).
