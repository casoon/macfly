#!/usr/bin/env bash
# Builds the deterministic castwright terminal demos (.terminal.yaml -> .cast + .svg)
# and showcase fixtures in assets/casts/.
#
# Usage:
#   assets/casts/build.sh
set -euo pipefail

dir="$(cd "$(dirname "$0")" && pwd)"
castwright="${CASTWRIGHT:-npx -y -p @casoon/castwright@0.6.0 -p shiki castwright}"

cat > "$dir/hero.terminal.yaml" <<'YAML'
version: 1
terminal:
  title: macfly · Rust + AppleScript/JXA
  cols: 64
  rows: 22
  theme: catppuccin-mocha
steps:
  - run: macfly doctor
  - wait: 400
  - output:
      raw: "\e[1;92m✔\e[0m macOS Automation Doctor\r\n  \e[2mPassed: \e[0m\e[1;92m6\e[0m   \e[2mAction required: \e[0m0\r\n  \e[92m✔\e[0m Notes · Mail · Contacts · Calendar · Reminders · Shortcuts\r\n\r\n"
  - wait: 900
  - run: macfly notes audit
  - wait: 450
  - output:
      raw: "\e[1;93m⚠\e[0m Notes Hygiene Audit (profile: default)\r\n  \e[2mNotes: \e[0m825   \e[2mFolders: \e[0m105   \e[2mEmpty: \e[0m\e[1;93m3\e[0m   \e[2mSingle-note: \e[0m\e[1;93m24\e[0m\r\n  \e[2mNext: \e[0m\e[36mmacfly plan create --file reorganize.json\e[0m\r\n\r\n"
  - wait: 900
  - run: macfly context person "Acme"
  - wait: 450
  - output:
      raw: "\e[1;92m✔\e[0m 360° Person & Project Dossier: 'Acme'\r\n  \e[2mHits: \e[0m\e[36m14\e[0m   \e[2mNotes: \e[0m5   \e[2mMail: \e[0m6   \e[2mCalendar: \e[0m2   \e[2mTasks: \e[0m1\r\n  \e[92m✔\e[0m DLP firewall active · 1 IBAN & 1 API key redacted\r\n"
  - wait: 4500
YAML

cat > "$dir/hero-narrow.terminal.yaml" <<'YAML'
version: 1
terminal:
  title: macfly · macOS automation
  cols: 50
  rows: 22
  theme: catppuccin-mocha
steps:
  - run: macfly doctor
  - wait: 400
  - output:
      raw: "\e[1;92m✔\e[0m macOS Automation Doctor\r\n  \e[2mPassed: \e[0m\e[1;92m6\e[0m   \e[2mBlocked: \e[0m0\r\n  \e[92m✔\e[0m Notes · Mail · Contacts · Calendar\r\n  \e[92m✔\e[0m Reminders · Safari · Shortcuts\r\n\r\n"
  - wait: 900
  - run: macfly notes audit
  - wait: 450
  - output:
      raw: "\e[1;93m⚠\e[0m Notes Hygiene Audit\r\n  \e[2mNotes: \e[0m825   \e[2mFolders: \e[0m105\r\n  \e[2mEmpty: \e[0m\e[1;93m3\e[0m     \e[2mSingle-note: \e[0m\e[1;93m24\e[0m\r\n\r\n"
  - wait: 900
  - run: macfly context person "Acme"
  - wait: 450
  - output:
      raw: "\e[1;92m✔\e[0m 360° Dossier: 'Acme'\r\n  \e[2mNotes: \e[0m5  \e[2mMail: \e[0m6  \e[2mCal: \e[0m2  \e[2mTasks: \e[0m1\r\n  \e[92m✔\e[0m DLP active · 2 secrets redacted\r\n"
  - wait: 4500
YAML

cat > "$dir/notes-cleanup.terminal.yaml" <<'YAML'
version: 1
terminal:
  title: Use Case 1 · Apple Notes Audit & SHA-256 Reorganization Plan
  cols: 76
  rows: 25
  theme: catppuccin-mocha
steps:
  - run: macfly notes audit
  - wait: 450
  - output:
      raw: "\e[1;93m⚠\e[0m Notes Hygiene Audit (profile: default)\r\n\r\n  \e[2mTotal notes: \e[0m825   \e[2mFolders: \e[0m105   \e[2mEmpty: \e[0m\e[1;93m3\e[0m   \e[2mSingle-note: \e[0m\e[1;93m24\e[0m\r\n\r\n  \e[1;96mSingle-note folders (consolidation candidates):\e[0m\r\n    \e[33m!\e[0m 'Q1 Taxes' (1 note) -> move to '10 · Work / Finance'\r\n    \e[33m!\e[0m 'Rust Ideas' (1 note) -> move to '20 · Engineering'\r\n    \e[2m…and 22 more single-note folders\e[0m\r\n\r\n"
  - wait: 1100
  - run: macfly plan create --file reorganize-notes.json
  - wait: 450
  - output:
      raw: "\e[1;96m!\e[0m Plan Created & Hash-Locked\r\n  \e[2mPlan ID: \e[0mf47ac10b-58cc-4372-a567-0e02b2c3d479\r\n  \e[2mOperations: \e[0m\e[36m27\e[0m   \e[2mSHA-256: \e[0ma1b2c3d4e5f60718   \e[2mApproval code: \e[0m\e[1;93ma1b2c3\e[0m\r\n  \e[2mFidelity Guard: \e[0m\e[92m✔\e[0m 19 notes with attachments/tables moved losslessly\r\n\r\n"
  - wait: 1100
  - run: macfly plan approve f47ac10b-58cc-4372-a567-0e02b2c3d479 --code a1b2c3
  - wait: 400
  - output:
      raw: "\e[1;92m✔\e[0m Plan Approved (TTY verified · SHA-256 integrity intact)\r\n\r\n"
  - wait: 700
  - run: macfly plan apply f47ac10b-58cc-4372-a567-0e02b2c3d479
  - wait: 500
  - output:
      raw: "\e[1;92m✔\e[0m Plan Applied\r\n  \e[2mSucceeded: \e[0m\e[1;92m27\e[0m   \e[2mFailed: \e[0m0   \e[2mJournal: \e[0m~/.local/share/macfly/journal.jsonl\r\n"
  - wait: 4500
YAML

cat > "$dir/migration-import.terminal.yaml" <<'YAML'
version: 1
terminal:
  title: Use Case 2 · Multi-Format Knowledge & Task Migration
  cols: 76
  rows: 24
  theme: catppuccin-mocha
steps:
  - run: macfly notes import --input ./obsidian-vault --folder "00 · Inbox" --recursive
  - wait: 450
  - output:
      raw: "\e[1;96m!\e[0m Multi-Format Notes Import Plan Created\r\n  \e[2mParsed files: \e[0m\e[36m42\e[0m (.md: 34, .html: 4, .json: 2, .csv: 2)\r\n  \e[2mTarget folder: \e[0m00 · Inbox   \e[2mPlan ID: \e[0m7c9e6679-7425-40de-944b-e07fc1f90ae7\r\n  \e[2mApproval code: \e[0m\e[1;93m8f4e2a\e[0m\r\n\r\n"
  - wait: 1100
  - run: macfly reminders import --input ./todoist-tasks.csv --list "Work"
  - wait: 450
  - output:
      raw: "\e[1;96m!\e[0m Reminders Import Plan Created\r\n  \e[2mTasks parsed: \e[0m\e[36m18\e[0m (RFC-4180 CSV with due_date, priority, notes)\r\n  \e[2mTarget list: \e[0mWork         \e[2mApproval code: \e[0m\e[1;93m3d9c1b\e[0m\r\n\r\n"
  - wait: 1100
  - run: macfly notes export --output ./git-knowledge-base --folder "10 · Work"
  - wait: 450
  - output:
      raw: "\e[1;92m✔\e[0m Notes Export Completed\r\n  \e[2mExported: \e[0m\e[1;92m64\e[0m Markdown files with YAML frontmatter (id, folder, revision)\r\n  \e[2mOutput: \e[0m./git-knowledge-base\r\n"
  - wait: 4500
YAML

cat > "$dir/context-dossier.terminal.yaml" <<'YAML'
version: 1
terminal:
  title: Use Case 3 · 360° Cross-App Client Dossier & Typst PDF Report
  cols: 76
  rows: 24
  theme: catppuccin-mocha
steps:
  - run: macfly context person "Acme"
  - wait: 450
  - output:
      raw: "\e[1;92m✔\e[0m 360° Person & Project Dossier: 'Acme'\r\n\r\n  \e[2mTotal hits: \e[0m\e[36m14\e[0m   \e[2mContacts: \e[0m2   \e[2mNotes: \e[0m5   \e[2mMail: \e[0m4   \e[2mCalendar: \e[0m2   \e[2mTasks: \e[0m1\r\n\r\n  \e[1;96mMatching Contacts (2):\e[0m\r\n    \e[36mℹ\e[0m Alex Rivera (Acme GmbH) · alex@acme.example · +49 30 5550192\r\n  \e[1;96mMatching Apple Notes (5):\e[0m\r\n    \e[36mℹ\e[0m [10 · Work] Acme Architecture & SLA Review (2026-11-18)\r\n  \e[1;96mRecent Emails (4):\e[0m\r\n    \e[36mℹ\e[0m [Exchange] Q4 Contract & Billing IBAN [REDACTED:IBAN] (2026-11-20)\r\n\r\n"
  - wait: 1200
  - run: macfly report person "Acme" --output ./dossier-acme.pdf
  - wait: 500
  - output:
      raw: "\e[1;92m✔\e[0m PDF Dossier Compiled (via Typst)\r\n  \e[2mPDF: \e[0m./dossier-acme.pdf   \e[2mSource: \e[0m./dossier-acme.typ\r\n  \e[2mTip: \e[0mUse --output .typ for renderreport or --output .md for Markdown\r\n"
  - wait: 4500
YAML

cat > "$dir/dlp-mail-triage.terminal.yaml" <<'YAML'
version: 1
terminal:
  title: Use Case 4 · Read-Side DLP Firewall & Safe Apple Mail Drafts
  cols: 76
  rows: 23
  theme: catppuccin-mocha
steps:
  - run: macfly mail search --query "Invoice" --limit 2
  - wait: 450
  - output:
      raw: "\e[1;92m✔\e[0m Mail Search: 'Invoice' (2 matches · DLP active · untrusted_external)\r\n\r\n  \e[36m1.\e[0m \e[1mInvoice #2026-114 · Wire Transfer Details\e[0m\r\n     \e[2mFrom: \e[0mbilling@acme.example   \e[2mAccount: \e[0mExchange\r\n     \e[2mBody: \e[0mWire EUR 4,200 to \e[1;93m[REDACTED:IBAN]\e[0m · key \e[1;93m[REDACTED:API_KEY]\e[0m\r\n\r\n"
  - wait: 1100
  - run: macfly mail signatures
  - wait: 400
  - output:
      raw: "Signatures from Apple Mail (Single Point of Truth):\r\n  • 'Corporate Standard' (native Apple Mail signature · 0.24s)\r\n\r\n"
  - wait: 900
  - run: macfly plan create --file draft-reply.json
  - wait: 450
  - output:
      raw: "\e[1;96m!\e[0m Plan Created & Hash-Locked (mail.create_draft · send_enabled: false)\r\n  \e[2mSignature: \e[0mCorporate Standard (from Apple Mail)   \e[2mApproval code: \e[0m\e[1;93m9c4a1e\e[0m\r\n"
  - wait: 4500
YAML

cat > "$dir/daily-automation.terminal.yaml" <<'YAML'
version: 1
terminal:
  title: Use Case 5 · Executive Daily Brief, launchd Schedule & SKILL.md
  cols: 76
  rows: 23
  theme: catppuccin-mocha
steps:
  - run: macfly context daily-brief
  - wait: 450
  - output:
      raw: "\e[1;92m✔\e[0m Executive Daily Brief\r\n  \e[2mCalendar (48h): \e[0m\e[36m4\e[0m   \e[2mOpen Tasks: \e[0m\e[1;93m7\e[0m   \e[2mUnread Mail: \e[0m\e[36m3\e[0m   \e[2mRecent Notes: \e[0m12\r\n\r\n"
  - wait: 1000
  - run: macfly schedule add morning-brief --every-minutes 60 -- context daily-brief
  - wait: 450
  - output:
      raw: "\e[1;92m✔\e[0m LaunchAgent Installed & Loaded\r\n  \e[2mLabel: \e[0mcom.casoon.macfly.morning-brief   \e[2mInterval: \e[0m60m\r\n  \e[2mPlist: \e[0m~/Library/LaunchAgents/com.casoon.macfly.morning-brief.plist\r\n\r\n"
  - wait: 1000
  - run: macfly skill export --output ./.claude/skills/macfly
  - wait: 400
  - output:
      raw: "\e[1;92m✔\e[0m Agent Skill Exported\r\n  \e[2mPath: \e[0m./.claude/skills/macfly/SKILL.md\r\n"
  - wait: 4500
YAML

for scene in hero hero-narrow notes-cleanup migration-import context-dossier dlp-mail-triage daily-automation; do
  $castwright build "$dir/$scene.terminal.yaml" -o "$dir"
  $castwright build "$dir/$scene.terminal.yaml" --format svg -o "$dir"
done
