---
title: Plans & Workflows
description: How MacFly's transactional Plan → Inspect → Approve → Apply lifecycle works, with JSON examples for all supported operations.
order: 2
---

MacFly never mutates your Apple Notes or Apple Mail directly from a single
unverified command. Every write follows four explicit steps:

1. **Create:** `macfly plan create --input plan.json` (or `--stdin`) validates
   all operations against your profile scopes, computes a canonical SHA-256
   `plan_hash` and 6-character `approval_code`, sets a TTL (default 15 minutes),
   and writes the plan to `~/.local/share/macfly/plans/` (`0600`).
2. **Inspect:** `macfly plan inspect <plan-id>` displays risk level,
   reversibility, hash, and human-readable previews.
3. **Approve:** `macfly plan approve <plan-id>` requires an interactive TTY and
   entering the 6-character confirmation code.
4. **Apply:** `macfly plan apply <plan-id>` re-verifies plan integrity, expiry,
   and optional note revisions, executes the operations, and appends a full
   record to `~/.local/share/macfly/journal.jsonl` (`macfly history`).

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
