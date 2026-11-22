---
title: Getting Started
description: Build MacFly, initialize your central config, verify macOS permissions, and run your first commands across Notes, Mail, Calendar, and Reminders.
order: 1
---

## 1. Install MacFly

### Direct binary installer (recommended — no Rust/Cargo required)

Downloads the prebuilt macOS binary (`aarch64-apple-darwin` or `x86_64-apple-darwin`) from GitHub Releases, verifies its SHA-256 checksum against `SHA256SUMS`, and installs to `~/.local/bin/macfly`:

```bash
curl -fsSL https://raw.githubusercontent.com/casoon/macfly/main/install.sh | bash
```

Optional environment overrides: `MACFLY_VERSION=v0.4.0` and `INSTALL_DIR=/usr/local/bin`.

### Via Cargo or from source

```bash
cargo install macfly
```

Or build from a local clone (`cargo`, Edition 2024 — no Xcode project or Node.js runtime required):

```bash
git clone https://github.com/casoon/macfly.git
cd macfly
cargo build --release
```

The single native binary is created at `target/release/macfly`.

## 2. Verify macOS permissions

```bash
macfly doctor
```

On first execution, macOS prompts once for Automation access (TCC) to **Apple
Notes** and **Apple Mail**. If a permission is missing, `macfly doctor` reports
`TCC_PERMISSION_DENIED` along with the exact System Settings path.

## 3. Single-Point-of-Truth configuration

```bash
macfly config init
```

This creates `~/.config/macfly/config.json` (overridable via `$MACFLY_CONFIG`
or `--config`). You can define multiple named profiles (e.g. `default`,
`work-readonly`) and keep your email signature in Apple Mail instead of
duplicating it:

```json
{
  "schema_version": "1.0",
  "default_profile": "default",
  "script_timeout_secs": 25,
  "profiles": {
    "default": {
      "notes": {
        "allow_accounts": [],
        "allow_folders": [],
        "deny_folders": ["Passwords", "Recently Deleted", "Private"],
        "skip_password_protected": true
      },
      "mail": {
        "allow_accounts": [],
        "deny_mailboxes": ["Junk", "Trash"],
        "signature": {
          "source": "apple-mail",
          "name": null
        }
      },
      "dlp": {
        "enabled": true,
        "redact": ["private_key", "api_key", "iban", "credit_card", "otp_code"],
        "max_body_chars": 12000
      }
    }
  }
}
```

## 4. Everyday commands

Every command supports `--format pretty` (human-readable terminal default),
`--format json` (pretty JSON), and `--format agent` (compact JSON for LLMs).

```bash
# Apple Notes
macfly notes overview
macfly notes audit
macfly notes list --limit 20
macfly notes search "Roadmap"
macfly notes read "<note-id>"
macfly notes export --output ./notes-backup --folder "10 · Work"
macfly notes import --input ./drafts --folder "00 · Inbox"

# Apple Mail
macfly mail overview
macfly mail signatures
macfly mail list --unread --limit 15
macfly mail search "Invoice"
macfly mail read "<local-id>"
macfly mail export --output ./mail-archive --query "Invoice" --limit 20

# Apple Calendar & Reminders
macfly calendar events --days 7
macfly reminders list --overdue

# Multi-app Context Packs & Typst/Markdown Dossiers
macfly context daily-brief --format agent
macfly context person "Acme"
macfly context meeting-prep --query "Kickoff" --create-note --folder "00 · Inbox"
macfly context weekly-review
macfly report person "Acme" --output ./dossier-acme.typ

# Export Agent SKILL.md & manage macOS launchd routines
macfly skill export --output ./.claude/skills/macfly
macfly schedule install --name morning-brief --task daily-brief --hour 7 --minute 30 --dry-run
macfly schedule list
```
