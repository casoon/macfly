---
title: Migrations & Backups
description: Two-way migration pipelines between Apple Notes, Mail, Contacts, Reminders, Calendar, Safari, and local Markdown, CSV, JSON, JSONL, or HTML repositories.
order: 3
---

Beyond connecting AI agents safely to macOS, **MacFly is a deterministic
migration and archival toolkit** for moving personal and project data between
Apple's native apps and open file formats (Git-backed Markdown knowledge bases,
Obsidian, DevMark, Notion, Evernote, Todoist, or CRM exports).

Every **export** produces clean Markdown files with structured YAML frontmatter,
and every **import** generates a tamper-evident **`macfly plan`** (SHA-256 hash +
approval + audit journal) rather than writing blindly. Imports with more than
5 notes or reminders always need your approval unless the profile uses the
`open` preset.

---

## Migration Matrix

| Domain | Export to Markdown + YAML Frontmatter | Import via Verified `macfly plan` |
|---|---|---|
| **Apple Notes** | `macfly notes export --output <dir>` (mirrors folder tree 1:1) | `macfly notes import --input <path> --folder <target> [--recursive]` (`.md`, `.txt`, `.html`, `.json`, `.jsonl`, `.csv`) |
| **Apple Mail** | `macfly mail export --output <dir>` (`--account`, `--mailbox`, `--query`, `--unread`) | `mail_create_draft` via `macfly plan create` (with Apple Mail signature) |
| **Apple Contacts** | `macfly contacts export --output <dir>` (`--query`, `--limit`) | *(Read, duplicate/empty audit & export)* |
| **Apple Reminders** | `macfly reminders export --output <file.md>` (`- [ ]` checklist) | `macfly reminders import --input <path> [--list <name>]` (`.md`, `.txt`, `.json`, `.jsonl`, `.csv`) |
| **Apple Calendar** | `macfly calendar export --output <file.md> [--days 14]` | *(Read, daily-brief & Markdown export)* |
| **Apple Safari** | `macfly safari export --output <file.md> [--query <q>]` | `macfly safari to-note --folder <target>` (captures open tabs into an Apple Note) |

---

## 1. Exporting out of Apple's Ecosystem (to Markdown / Git)

### Apple Notes → Markdown folder tree

```bash
# Export an entire folder branch with DLP redaction active
macfly notes export --output ./kb-notes --folder "10 · Work" --limit 500

# Export a full unredacted private backup
macfly notes export --output ./private-backup --limit 2000 --no-redact
```

Each note is saved at `<output>/<folder_path>/<title>.md` with YAML frontmatter:

```markdown
---
title: "Q4 Architecture Review"
id: "x-coredata://.../ICNote/p412"
account: "iCloud"
folder: "10 · Work/Architecture"
modified_at: "2026-11-22T12:10:00Z"
revision: "sha256:9f86d081884c7d65"
---

Clean note body...
```

### Apple Mail → Project email archive

```bash
# Archive all emails matching a client or invoice query
macfly mail export --output ./client-archive --query "Acme" --limit 50

# Export all unread inbox messages for offline/local-LLM triage
macfly mail export --output ./unread-triage --unread --limit 25
```

### Apple Contacts → Markdown contact cards

```bash
macfly contacts audit
macfly contacts export --output ./contacts-kb --query "GmbH"
```

### Apple Reminders, Calendar & Safari Tabs

```bash
macfly reminders export --output ./open-tasks.md
macfly calendar export --output ./next-2-weeks.md --days 14
macfly safari export --output ./open-research-tabs.md
```

---

## 2. Importing into Apple Notes & Reminders (Multi-Format)

MacFly's built-in Rust parser (`macfly_core::import`) supports **six input
formats** out of the box — with zero external runtime dependencies:

| Format | Extensions | Recognized Structure | Typical Sources |
|---|---|---|---|
| **Markdown** | `.md`, `.markdown` | YAML frontmatter (`title:`, `folder:`), leading `# Heading`, or `- [ ]` task lines | Obsidian, Notion Markdown, Bear, DevMark |
| **Plaintext** | `.txt` | Leading `# Heading` or filename (Notes); line-by-line tasks (Reminders) | Local text files, scratch lists |
| **HTML** | `.html`, `.htm` | `<title>` or `<h1>` + automatic HTML-to-clean-text conversion | Evernote, Google Keep, Confluence, web clips |
| **JSON** | `.json` | Single object, `[...]` array, or `{"notes": [...]}` / `{"tasks": [...]}` (auto-maps `title`/`name`/`subject`/`task` & `body`/`content`/`text`/`notes`) | Custom exports, API dumps |
| **JSON Lines** | `.jsonl` | One JSON note or task object per line | Batch pipelines, LLM outputs |
| **CSV (RFC 4180)** | `.csv` | Header detection (`title`/`name`/`task`, `body`/`content`/`notes`/`description`, `folder`/`list`/`project`), including multiline quoted fields | Notion databases, Airtable, Trello, Todoist, Excel/Numbers |

### Example: Migrate a folder of Markdown/HTML/CSV files into Apple Notes

```bash
# Recursively scan a directory and generate a verified change plan
macfly notes import --input ./obsidian-vault --folder "00 · Inbox" --recursive

# Or import a Notion / Airtable CSV export
macfly notes import --input ./notion-database.csv --folder "30 · Research"
```

MacFly prints the generated `plan_id` and its status. Review, approve
(if `pending_approval`); approving also applies it:

```bash
macfly plan inspect <plan-id>
macfly plan approve <plan-id>
```

### Example: Migrate tasks from Todoist / Trello CSV or Markdown into Apple Reminders

```bash
macfly reminders import --input ./todoist-export.csv --list "Work"
macfly reminders import --input ./roadmap-checklist.md --list "Projects"
```

### Example: Export photos from Apple Photos

`macfly photos export` always goes through a plan. It exports into a new
folder and never overwrites a file, so running it twice is safe.

```sh
macfly photos albums
macfly photos list --album "Receipts" --since 2024-01-01

# Originals plus an XMP file per photo (title, description, keywords, date, location)
macfly photos export --album "Receipts" --to ~/Pictures/Export --originals --sidecar xmp

# Rendered copies, converted with the built-in `sips`: JPEG, at most 2048 px, quality 85
macfly photos export --keyword invoice --to ~/Pictures/Web \
  --convert jpeg --max-size 2048 --quality 85
```

- `--originals` exports the file as imported (HEIC, RAW, …); without it Photos renders JPEGs with your edits.
- `--sidecar xmp|json|both` writes the metadata Photos exposes to scripts next to each file. Photos' own "export IPTC as XMP" switch is not reachable from a script, so the XMP is written by MacFly.
- `--convert` runs on the exported copies only and never touches your library. It only shrinks larger images, and videos are copied unchanged.
- Destinations must be below your home folder (not `~/Library`) unless `export.allow_dirs` in the config says otherwise.
- Each run writes a `macfly-export-<id>.sha256` manifest that `shasum -a 256 -c` can verify.

### Example: Capture open Safari tabs directly into an Apple Note

When a research session leaves 30 open tabs in Safari, capture them into a
single structured Apple Note via plan:

```bash
macfly safari to-note --folder "30 · Research" --title "Rust & macOS Research Links"
```
