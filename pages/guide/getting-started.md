---
title: Getting Started
description: Build MacFly, initialize your central config, verify macOS permissions, and run your first commands across Notes, Mail, Calendar, and Reminders.
order: 1
---

## 1. Build from source

Requires Rust (`cargo`, Edition 2024) on macOS. No Xcode project or Node.js
runtime is required.

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

# Apple Mail
macfly mail overview
macfly mail signatures
macfly mail list --unread --limit 15
macfly mail search "Invoice"
macfly mail read "<local-id>"

# Apple Calendar & Reminders
macfly calendar events --days 7
macfly reminders list --overdue

# Multi-app Context Pack (Calendar + Reminders + Unread Mail + Recent Notes)
macfly context daily-brief --format agent
```
