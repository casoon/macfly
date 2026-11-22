# macfly

Local, deterministic macOS automation CLI, migration toolkit, and `stdio` MCP server in pure Rust — with a read-side data firewall and cryptographic plan approval between AI agents and your personal apps.

**Documentation:** [casoon.github.io/macfly](https://casoon.github.io/macfly/)

## What it does

- **Apple Notes** — hierarchical folder overview, cleanup audit (empty folders, single-note folders, duplicate titles, oldest notes), search, cleaned reading with a **fidelity guard** (protects attachments, tables, checklists, and `applenotes://` links), lossless folder/note moves, **Markdown export**, and **multi-format import** (`.md`, `.txt`, `.html`, `.json`, `.jsonl`, `.csv` with `--recursive`)
- **Apple Mail** — account and mailbox overview, direct signature lookup from Apple Mail (single source of truth), search, quote-trimmed reading with `untrusted_external` framing, safe draft creation (never sends unattended), and **Markdown email archiving (`mail export`)**
- **Apple Contacts** — fast batched contact search, duplicate & empty-contact audit (`contacts audit`), and **Markdown contact card export (`contacts export`)**
- **Apple Calendar & Reminders** — upcoming events, open/overdue reminders, unified 4-app `daily-brief` context pack, **Calendar/Reminders Markdown export**, and **multi-format Reminders import** (`.md`, `.txt`, `.json`, `.jsonl`, `.csv`)
- **Apple Safari** — inspect open browser tabs (`safari tabs`), export tabs to Markdown (`safari export`), or migrate open research tabs directly into an Apple Note via plan (`safari to-note`)
- **Apple Shortcuts** — allowlist-gated bridge to `shortcuts list` and `shortcuts run` (Focus modes, HomeKit scenes, modern App Intents)
- **Read-side Data Firewall (DLP)** — profile-based allowlists/denylists (`deny_folders`, `deny_mailboxes`) plus automatic linear-time regex redaction of private keys, API keys, IBANs, credit cards, and 6-digit OTP codes before output reaches any LLM
- **Transactional change plans & `runemark` terminal UI** — every write goes through `plan create → inspect → approve → apply` with canonical SHA-256 plan hashing, tamper detection, expiry TTL, interactive TTY confirmation, a local JSONL audit journal, and semantic terminal reports powered by [`runemark`](https://casoon.github.io/runemark/)
- **Built-in `stdio` MCP server** — exposes 15 read & `plan_create`/`plan_inspect` tools; `plan_approve` and `plan_apply` are intentionally excluded from MCP so agents can propose changes but never self-approve them

## Why Rust + AppleScript/JXA

- **Deterministic core:** Policy evaluation, DLP redaction, HTML/email normalization, multi-format import parsers, fidelity checks, plan hashing, and audit journaling live in pure, memory-safe Rust and run offline in milliseconds via `cargo test`.
- **Zero SDK / Xcode drift:** No Swift toolchain, no Xcode project, no Node.js runtime, and no fragile private SQLite assumptions in the default path.
- **Safe, batched JXA edge:** All scripts are statically embedded (`include_str!`), receive parameters strictly as JSON over `stdin` (no `argv` leakage in `ps aux`), use batched property fetches, and enforce hard timeouts with typed TCC error hints.

## Requirements

- **macOS** (Apple Silicon or Intel)
- **Rust** toolchain (`cargo`, Edition 2024)
- macOS Automation (TCC) permission for the apps you use (prompted automatically on first run; verified via `macfly doctor`)

## Installation & quick start

```sh
cargo install macfly
macfly doctor
macfly config init
```

Everyday commands (all support `--format pretty`, `--format json`, and `--format agent`, plus `--color auto|always|never`):

```sh
# Apple Notes (inspect, audit, export & multi-format import)
macfly notes overview
macfly notes audit
macfly notes list --limit 20
macfly notes search "Roadmap"
macfly notes read "<note-id>"
macfly notes export --output ./notes-backup --folder "10 · Work"
macfly notes import --input ./evernote-or-notion --folder "00 · Inbox" --recursive

# Apple Mail (inspect, signatures, read & Markdown export)
macfly mail overview
macfly mail signatures
macfly mail list --unread --limit 15
macfly mail read "<local-id>"
macfly mail export --output ./mail-archive --query "Invoice" --limit 20

# Apple Contacts (search, audit & Markdown export)
macfly contacts list --limit 50
macfly contacts search "Acme"
macfly contacts audit
macfly contacts export --output ./contacts-backup

# Apple Calendar & Reminders (events, tasks, export & multi-format import)
macfly calendar events --days 7
macfly calendar export --output ./upcoming-events.md --days 14
macfly reminders list --overdue
macfly reminders export --output ./open-tasks.md
macfly reminders import --input ./tasks.csv --list "Work"

# Apple Safari & Apple Shortcuts
macfly safari tabs
macfly safari export --output ./research-tabs.md
macfly safari to-note --folder "00 · Inbox" --title "Research Links"
macfly shortcuts list

# Multi-app context pack
macfly context daily-brief --format agent
```

## Configuration

MacFly uses a single configuration file at `~/.config/macfly/config.json` (override with `--config` or `$MACFLY_CONFIG`). Email accounts stay in Apple Mail, and signatures default to `{"source": "apple-mail"}` so your email footer has a single source of truth.

See [Getting Started](https://casoon.github.io/macfly/docs/guide/getting-started/) and [Migrations & Backups](https://casoon.github.io/macfly/docs/guide/migrations/) for the full configuration schema and migration workflows.

## Plans & audit journal

```sh
macfly plan create --input plan.json
macfly plan inspect <plan-id>
macfly plan approve <plan-id>
macfly plan apply <plan-id>
macfly history
```

Plans are stored at `~/.local/share/macfly/plans/` (`0600`) and executions are logged to `~/.local/share/macfly/journal.jsonl` (`0600`). See [Plans & Workflows](https://casoon.github.io/macfly/docs/guide/plans-and-workflows/) for all supported operations.

## MCP server

Point any MCP-compatible client (Claude Desktop, Cursor, Antigravity, or local LLM runners) to the same binary:

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

See [MCP & AI Agents](https://casoon.github.io/macfly/docs/guide/mcp-and-agents/) and [Security Model](https://casoon.github.io/macfly/docs/architecture/security-model/) for details.

## License

Dual-licensed under **MIT** or **Apache-2.0**, at your option.
