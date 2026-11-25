---
title: Plans & Workflows
description: How MacFly's transactional Plan → Inspect → Approve → Apply lifecycle works, with JSON examples for all supported operations.
order: 2
---

MacFly never mutates your Apple Notes or Apple Mail directly from a single
unverified command. Every write is a plan with up to four steps — how many of
them need you depends on the profile's [security preset](#security-presets).

1. **Create:** `macfly plan create --input plan.json` (or `--stdin`, or
   `plan_create` over MCP) validates all operations against your profile scopes,
   computes a canonical SHA-256 `plan_hash`, sets a TTL (default 15 minutes),
   and writes the plan to `~/.local/share/macfly/plans/` (`0600`). If the preset
   allows every operation, the plan is created as `approved`
   (`approved_by: "preset:balanced"`); otherwise it waits as `pending_approval`.
2. **Inspect:** `macfly plan inspect <plan-id>` displays status, risk level,
   reversibility, hash, and **full-content previews** (complete Apple Mail draft
   bodies, line-by-line Apple Notes additions, and high-risk Shortcut warnings).
3. **Approve (only for `pending_approval`):** `macfly plan approve <plan-id>`
   requires an interactive terminal, shows the full preview and asks you to type
   a random 6-character code that is only shown there — never in plan output,
   JSON or MCP responses.
4. **Apply (with lock & `--resume`):** `macfly plan apply <plan-id>` (or
   `plan_apply` over MCP) runs only approved plans. It acquires an
   exclusive `.lock` file against duplicate runs, re-verifies plan integrity and
   optional note revisions, executes operations sequentially while persisting
   progress after each step, and appends a record to
   `~/.local/share/macfly/journal.jsonl` (`macfly history`). If a multi-step plan
   stops mid-way, it is marked `partially_applied` and can be continued cleanly
   with `macfly plan apply <plan-id> --resume` (note: Apple app mutations are
   resumable, not full atomic rollbacks).

## Security Presets

Each profile has a `preset` that decides which plans run without asking you.
`macfly config init` asks once; without a config, `balanced` applies.
`macfly policy` explains the active rules in plain language.

| Preset | Runs automatically | Needs your approval |
|---|---|---|
| `strict` | nothing | every change |
| `balanced` (default) | `notes_create`, `notes_create_folder`, `mail_create_draft`, `mail_mark_read`, `reminders_create` — up to 5 operations per plan | `notes_append`, `notes_move`, `notes_move_folder_contents`, `notes_delete_empty_folder`, `mail_move`, `shortcuts_run`, and any plan with more than 5 operations |
| `open` | additionally `notes_append`, `notes_move`, `notes_move_folder_contents`, `notes_delete_empty_folder`, `mail_move` — in any number | `shortcuts_run` |

In every preset, scope rules (`deny_folders`, `deny_mailboxes`, allowlists) and
DLP redaction apply, mails are only ever saved as drafts, and every applied plan
is journaled. `--no-redact` only works in an interactive terminal.

```json
{
  "default_profile": "default",
  "profiles": {
    "default": { "preset": "balanced" },
    "mcp": { "preset": "strict" }
  }
}
```

## Supported Operations

### Apple Notes

```json
{
  "summary": "Organize project notes and clean empty folder",
  "operations": [
    {
      "action": "notes_create_folder",
      "account": "iCloud",
      "name": "Projects",
      "parent_folder": "10 · Work"
    },
    {
      "action": "notes_move",
      "note_id": "x-coredata://.../ICNote/p123",
      "expected_revision": "sha256:9f86d081884c7d65",
      "destination_folder": "10 · Work/Projects"
    },
    {
      "action": "notes_move_folder_contents",
      "source_folder_id": "x-coredata://.../ICFolder/p45",
      "source_folder_path": "Old Inbox",
      "destination_folder_path": "00 · Inbox"
    },
    {
      "action": "notes_create",
      "folder": "10 · Work/Projects",
      "title": "Weekly Summary",
      "body": "Key milestones and open questions."
    },
    {
      "action": "notes_append",
      "note_id": "x-coredata://.../ICNote/p123",
      "expected_revision": "sha256:9f86d081884c7d65",
      "text": "Follow-up: confirm release date."
    },
    {
      "action": "notes_delete_empty_folder",
      "folder_id": "x-coredata://.../ICFolder/p45",
      "folder_name": "Old Inbox"
    }
  ]
}
```

> **Safety guarantees:** `notes_move` and `notes_move_folder_contents` move
> native note objects losslessly (preserving attachments, tables, checklists,
> and `applenotes://` links). `notes_delete_empty_folder` aborts if the folder
> still contains any note or any active subfolder.

### Apple Mail

```json
{
  "summary": "Prepare reply draft and mark processed mail as read",
  "operations": [
    {
      "action": "mail_create_draft",
      "to": ["partner@example.org"],
      "cc": [],
      "subject": "Re: Project timeline",
      "body": "Thanks for the update — the proposed dates work well for us."
    },
    {
      "action": "mail_mark_read",
      "local_id": "12345",
      "account": "Work",
      "mailbox": "INBOX",
      "read": true
    },
    {
      "action": "mail_move",
      "local_id": "12345",
      "account": "Work",
      "from_mailbox": "INBOX",
      "to_mailbox": "Archive"
    }
  ]
}
```

> **Mail signatures:** `mail_create_draft` automatically attaches your
> configured Apple Mail signature (`signature.source = "apple-mail"`) and opens
> the message as a **draft** for human review — MacFly never sends emails
> unattended.
