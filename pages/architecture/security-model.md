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
| **T1** | Indirect prompt injection (Write) | Malicious email instructs an agent to forward messages, delete notes, or alter data | Agents can only submit *proposals* (`plan_create`). The profile's [security preset](#security-presets) lets only low-risk, reversible plans through automatically; everything else waits for interactive approval. Mail is never sent, only drafted. |
| **T2** | Read-side data exfiltration | Broad search sends passwords, IBANs, 2FA codes, or private folders to a cloud LLM | Profile-level `allow_accounts` / `deny_folders` / `deny_mailboxes` plus automatic regex DLP redaction (`[REDACTED:IBAN_1]`, etc.) before output. |
| **T3** | Self-approval | Agent approves its own plan | MCP has no `plan_approve`; `plan_apply` only runs plans that are already approved. On the CLI, `macfly plan approve` requires an interactive terminal and a random code that never appears in plan output, JSON or MCP. See [Honest System Boundary](#honest-system-boundary) for agents with shell access. |
| **T4** | Plan tampering & TOCTOU | Stored plan is edited between review and apply, or a note changed meanwhile | Plans are stored at mode `0600` with a canonical SHA-256 hash and expiry TTL. `load()` re-verifies the hash; `notes_append` / `notes_move` verify `expected_revision` pre-flight. |
| **T5** | Script & argument injection | Special characters break out of AppleScript, or secrets leak in `ps aux` | Statically embedded JXA scripts (`include_str!`); all inputs passed strictly as JSON via `stdin`. |
| **T6** | Silent data loss on rich notes | Setting HTML body overwrites attachments, tables, checklists, or `applenotes://` links | `analyze_note_fidelity` inspects every note and blocks `replace_body` when rich content is present; moves operate on native note objects losslessly. |
| **T7** | Unchecked irreversible actions | Sending an email or deleting a non-empty folder cannot be undone | Mail mutations create **drafts** for review (no blind send); `notes_delete_empty_folder` re-verifies zero notes and zero active subfolders live before deleting. |

## Security Presets

Instead of a long list of individual rules, each profile picks one of three
presets (`macfly config init` asks once, `macfly policy` explains the result):

- **`strict`** — every change needs your approval in the terminal. For
  sensitive mailboxes and company machines.
- **`balanced`** (default) — creating notes, folders, mail drafts and
  reminders, and marking mail as read, runs automatically (up to 5 operations
  per plan). Moving, appending, deleting and Shortcuts need your approval.
- **`open`** — additionally moving, appending and deleting empty folders runs
  automatically, in any number. For cleanup and migration sessions. Shortcuts
  still need your approval; `macfly doctor` warns while `open` is active.

Presets only change *who approves*. Scopes, DLP, fidelity checks, the
draft-only mail rule and the audit journal apply in every preset. Auto-approved
plans are recorded with `approved_by: "preset:<name>"`, human approvals with
`approved_by: "human"`.

## Privacy by Deployment: Local AI vs. Cloud Models

Because MacFly speaks plain JSON over CLI and `stdio` MCP without tying you to
a specific provider, the strongest defense against cloud data leakage is
architectural: **pair MacFly with a local model (Ollama, LM Studio, MLX,
`llama.cpp`) for private notes, mail triage, and local workflows.** Nothing
leaves your Mac.

When cloud models are used, MacFly's read-side Data Firewall (DLP) and strict
profile scopes reduce exposure to only what you explicitly allow.

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
   - `credit_card` (13–19 digit card numbers with or without separators, only
     if the Luhn checksum is valid)
   - `otp_code` (context-bound 6-digit verification / TAN / 2FA codes)
   - `password` (the value after `Passwort:`, `Kennwort:`, `Password=`, `PIN:`
     and similar keywords; the keyword itself stays readable)

   The `redact` list in a profile is explicit: a config written before a rule
   existed only gets it after the rule name is added to `redact`.
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

MacFly reliably protects every action routed through MacFly, and for MCP-only
clients without a shell (e.g. Claude Desktop) the approval boundary holds.

An agent with **unrestricted shell access in the same macOS user session**
(Claude Code, Codex, Cursor) is a different case. It can edit the MacFly config
or plan files, fake an interactive terminal, or invoke `osascript` directly if
its terminal has Automation (TCC) permissions. For such agents, presets and
approvals are guardrails that keep a cooperative agent — and an agent misled by
prompt injection that stays on the MacFly path — on safe ground; they are not a
sandbox. Pair them with your agent host's own permission rules, for example in
Claude Code's `settings.json`:

```json
{
  "permissions": {
    "allow": ["Bash(macfly:*)"],
    "deny": [
      "Bash(macfly plan approve:*)",
      "Bash(osascript:*)",
      "Bash(script:*)",
      "Edit(~/.config/macfly/**)",
      "Edit(~/.local/share/macfly/**)"
    ]
  }
}
```

True OS-level enforcement requires privilege separation: running the agent
without TCC Automation permissions and routing requests over a local Unix socket
to a separately signed **`macfly-broker`** helper holding the TCC grant and
prompting via macOS Touch ID (`LAContext`). This is the next major milestone on
the [Roadmap](/macfly/docs/architecture/roadmap/).
