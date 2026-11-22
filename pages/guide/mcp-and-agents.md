---
title: MCP & AI Agents
description: Connect MacFly to Claude Desktop, Cursor, Codex, or custom agents using the built-in stdio MCP server or compact CLI output.
order: 3
---

MacFly gives you two clean integration paths from the **exact same binary**:

1. **CLI-first (`--format agent`):** Ideal for terminal agents, local LLM
   pipelines, and shell scripts — zero daemon overhead, works directly with
   pipes and files.
2. **Built-in `stdio` MCP server (`macfly mcp`):** Ideal for desktop clients
   like Claude Desktop, Cursor, Antigravity, or local MCP-capable runners.

## First-class use case: Local AI (Zero Cloud Transfer)

MacFly is **completely model-independent** — it does not bundle or require any
cloud API key. That makes it a natural fit for **local LLMs running on Apple
Silicon** (such as Ollama, LM Studio, `llama.cpp`, or MLX):

- Your emails, personal notes, calendar events, and reminders **never leave your
  Mac** — no accidental data transfer to ChatGPT, Claude, or other cloud
  providers.
- Even when you *do* choose a cloud model for specific tasks, MacFly's
  read-side DLP firewall and `deny_folders` / `deny_mailboxes` policies act as
  an explicit safety net before any text leaves your machine.

## MCP client configuration

### Claude Code

```bash
claude mcp add macfly -- macfly mcp --profile default
```

### OpenAI Codex CLI

```bash
codex mcp add macfly -- macfly mcp --profile default
```

Or add to `~/.codex/config.toml`:

```toml
[mcp_servers.macfly]
command = "macfly"
args = ["mcp", "--profile", "default"]
```

### Claude Desktop & Cursor (`mcpServers` JSON)

```json
{
  "mcpServers": {
    "macfly": {
      "command": "macfly",
      "args": ["mcp", "--profile", "default"]
    }
  }
}
```

## Available MCP tools (19 total)

| Tool | Scope | Purpose |
|---|---|---|
| `notes_overview` | Read | Accounts & hierarchical folder paths with note counts |
| `notes_audit` | Read | Empty folders, 1-note folders, duplicate titles, oldest notes |
| `notes_list` | Read | Filterable/searchable note metadata |
| `notes_read` | Read | Cleaned note text + DLP redaction + fidelity & revision hash |
| `mail_overview` | Read | Accounts, mailboxes (unread counts), Apple Mail signatures |
| `mail_list` | Read | Filterable/searchable inbox & mailbox messages |
| `mail_read` | Read | Cleaned mail body + DLP redaction + `untrusted_external` framing |
| `contacts_search` | Read | Search or list Apple Contacts (name, org, emails, phones) |
| `contacts_audit` | Read | Duplicate names, shared emails, and contacts without email/phone |
| `calendar_events` | Read | Upcoming events for the next N days |
| `reminders_list` | Read | Open or overdue reminders |
| `safari_tabs` | Read | Currently open Safari browser tabs (title & URL) |
| `shortcuts_list` | Read | Available macOS Shortcuts & allowlist status |
| `context_daily_brief` | Read | 4-app morning briefing (events 48h, reminders, unread mails, recent notes) |
| `context_person` | Read | 360° dossier for a person/company across Contacts, Mail, Notes, Calendar & Reminders |
| `context_meeting_prep` | Read | Prepare for upcoming/matching calendar event with related contacts, mails & notes |
| `context_weekly_review` | Read | Weekly review of modified notes, unread mails, overdue tasks, events & Safari tabs |
| `plan_create` | Propose | Validate and store a proposed change plan (no execution) |
| `plan_inspect` | Read | Inspect status, hash, and preview of a stored plan |

## Ready-to-use `SKILL.md` export

For terminal agents (Claude Code, Codex, Cursor, Antigravity), export a
tailored `SKILL.md` with one command:

```bash
macfly skill export --output ./.claude/skills/macfly
```

## Why `approve` and `apply` are missing from MCP

This is intentional. Even if an agent is tricked by indirect prompt injection
inside an incoming email, it **cannot** approve or apply a plan through MCP.
The agent proposes a `plan_id`; you review and approve it in your own terminal:

```bash
macfly plan inspect <plan-id>
macfly plan approve <plan-id>
macfly plan apply <plan-id>
```
