---
title: Cleaning Up & Migrating Notes
description: Structure-agnostic field guide for reorganizing Apple Notes libraries and migrating between Apple Notes, Apple Mail, and Markdown knowledge bases.
order: 4
---

Large Apple Notes libraries tend to grow organically into dozens or hundreds of
deeply nested folders, single-note folders, duplicate subfolder names, and
orphaned scratchpads. Reorganizing them manually is tedious, and doing it with
naive scripts risks losing attachments or deleting non-empty folders.

This field guide summarizes practical patterns learned from real-world
migrations with MacFly. It is **completely independent of which organizational
system you prefer** — whether you use PARA, Johnny.Decimal, topic-based folders,
or a flat archive.

## The 4-Phase Safe Migration Method

### Phase 1: Audit metadata before touching anything

Start with a read-only inventory so you see the actual shape of your library
without sending note bodies to any model:

```bash
macfly notes overview
macfly notes audit
```

`notes audit` highlights the highest-leverage cleanup targets automatically:

- **Empty folders** (zero notes *and* zero active subfolders)
- **Single-note folders** (often signs of over-fragmentation)
- **Duplicate folder names** across different branches (e.g. multiple `Tasks` or
  `Templates` folders)
- **Duplicate note titles** (often caused by repeated captures or Smart Folders)
- **Oldest unmodified notes** (candidates for archiving)

### Phase 2: Protect sensitive areas first

Before letting an AI agent inspect note contents or suggest a reorganization,
add sensitive folders (credentials, taxes, health, personal) to `deny_folders`
in `~/.config/macfly/config.json`:

```json
"deny_folders": ["Passwords", "Taxes", "Health", "Recently Deleted"]
```

> **Tip:** Moving a whole folder with `notes_move_folder_contents` only touches
> folder metadata and native note objects — no model ever needs to read the
> contents of sensitive notes just to relocate them into a cleaner folder.

### Phase 3: Build the new structure in parallel, then move in batches

Avoid renaming and reshuffling old folders in place. Instead:

1. **Create the target folders first** in one `macfly plan` using
   `notes_create_folder` (idempotent — safe to re-run).
   - Keep nesting shallow (**max. 2 levels** works well across macOS and iOS).
   - If you want deterministic top-level ordering across Mac, iPhone, and iPad,
     use numeric prefixes (e.g. `00 ·`, `10 ·`, `20 ·`, `90 ·`) since Apple
     Notes sorts top-level folders alphabetically.
2. **Move contents folder by folder** using `notes_move_folder_contents` in
   small batches (10–15 folders per plan).
   - Because MacFly uses Apple Notes' native `move` operation, **attachments,
     tables, checklists, images, and `applenotes://` internal links stay 100 %
     intact.**
   - After each batch, run `macfly notes overview` and verify that the **total
     note count is unchanged** (`N → N`, zero notes lost).

### Phase 4: Clean up empty legacy folders bottom-up

Only after all notes are verified in their new homes, remove the now-empty old
folders using `notes_delete_empty_folder` — starting from the deepest leaf
folders and working upward in rounds.

MacFly enforces two live guards before deleting any folder:
- `notes.length == 0`
- zero active child folders referencing it as their container

If anything is still inside, MacFly aborts safely rather than deleting data.

---

## Real-World Apple Notes Edge Cases to Know

1. **Smart Folders (tag-based folders) are virtual views:**
   Smart Folders appear in Apple Notes' folder list, but their notes are only
   virtual matches (for example on `#task`). Moving notes out of a Smart Folder
   does not empty it — and because it still reports notes, MacFly will never
   accidentally delete it.
2. **Locked and shared notes stay put:**
   Password-protected or externally shared notes cannot be relocated via
   automation. Expect their source folder to keep `remaining: 1` after a batch
   move; handle those manually in Apple Notes.
3. **System folders cannot be removed:**
   `Notes` (default capture) and `Quick Notes` are built-in Apple system
   folders. Treat `Notes` as your raw capture inbox rather than trying to fight
   it.
4. **Rich-note edits vs. lossless moves:**
   Moving a note object is lossless. Replacing a note's HTML body via scripting
   is *not* — it destroys attachments and checklists. MacFly's fidelity guard
   therefore separates safe `move` / `append` operations from destructive body
   replacement.
5. **macOS folder-count caching during cleanup:**
   Immediately after deleting child folders, Apple Notes' cached
   `folder.folders.length` can lag behind reality. MacFly resolves active
   subfolders from live container references so bottom-up cleanup converges
   cleanly across rounds.

## Keeping It Clean Over Time

Once your library is reorganized, run a quick monthly check:

```bash
macfly notes audit
```

Catching new single-note folders, empty folders, and duplicate titles early
takes two minutes — and keeps your Notes library fast, searchable, and easy for
both humans and AI agents to navigate.

---

## Cross-System Migrations (Apple Notes & Mail ⇄ Markdown / Git)

Beyond reorganizing folders inside Apple Notes, MacFly supports deterministic
migrations between Apple's native apps and local Markdown repositories (such as
Git-backed knowledge bases, Obsidian, or static documentation):

### 1. Exporting Apple Notes to Markdown + YAML frontmatter

```bash
# Export an entire folder tree (mirrors subfolder structure 1:1)
macfly notes export --output ./kb-export --folder "10 · Work" --limit 500
```

Each note is written as `<title>.md` with structured YAML frontmatter (`title`,
`id`, `account`, `folder`, `modified_at`, `revision`). By default, DLP
redaction remains active; pass `--no-redact` when creating a private local
backup.

### 2. Archiving project emails to Markdown

```bash
# Export project or invoice emails as cleaned Markdown files
macfly mail export --output ./project-mails --query "Project Alpha" --limit 50
```

Each email becomes `<date>-<local_id>-<subject>.md` with frontmatter (`subject`,
`from`, `to`, `cc`, `date_received`, `account`, `mailbox`, `trust_level`) and
quote-trimmed body text.

### 3. Importing Markdown files into Apple Notes via Plan

When migrating notes *into* Apple Notes (for example from a Markdown folder or
another note tool's export) so they sync across Mac, iPhone, and iPad:

```bash
macfly notes import --input ./markdown-notes --folder "00 · Inbox"
```

Instead of writing blindly, `notes import` parses frontmatter titles (or top
`# Heading` lines) and creates a **tamper-evident `macfly plan`** that you
inspect, approve, and apply like any other change.
