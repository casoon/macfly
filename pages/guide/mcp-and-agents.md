---
title: MCP & AI Agents
description: Connect MacFly to Claude Desktop, Cursor, Codex, or custom agents using the built-in stdio MCP server or compact CLI output.
order: 3
---

MacFly gives you two clean integration paths from the **exact same binary**:

1. **CLI-first (`--format agent`):** Ideal for terminal agents (Codex, Claude
   Code, Gemini CLI) and shell scripts — zero daemon overhead, works directly
   with pipes and files.
2. **Built-in `stdio` MCP server (`macfly mcp`):** Ideal for desktop clients
   like Claude Desktop, Cursor, or Antigravity.

## MCP client configuration

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

## Available MCP tools (11 total)

| Tool | Scope | Purpose |
|---|---|---|
| `notes_overview` | Read | Accounts & hierarchical folder paths with note counts |
| `notes_audit` | Read | Empty folders, 1-note folders, duplicate titles, oldest notes |
| `notes_list` | Read | Filterable/searchable note metadata |
| `notes_read` | Read | Cleaned note text + DLP redaction + fidelity & revision hash |
| `mail_overview` | Read | Accounts, mailboxes (unread counts), Apple Mail signatures |
| `mail_list` | Read | Filterable/searchable inbox & mailbox messages |
| `mail_read` | Read | Cleaned mail body + DLP redaction + `untrusted_external` framing |
| `calendar_events` | Read | Upcoming events for the next N days |
| `reminders_list` | Read | Open or overdue reminders |
| `plan_create` | Propose | Validate and store a proposed change plan (no execution) |
| `plan_inspect` | Read | Inspect status, hash, and preview of a stored plan |

## Why `approve` and `apply` are missing from MCP

This is intentional. Even if an agent is tricked by indirect prompt injection
inside an incoming email, it **cannot** approve or apply a plan through MCP.
The agent proposes a `plan_id`; you review and approve it in your own terminal:

```bash
macfly plan inspect <plan-id>
macfly plan approve <plan-id>
macfly plan apply <plan-id>
```
