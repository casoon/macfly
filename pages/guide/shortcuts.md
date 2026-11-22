---
title: Apple Shortcuts & Automation
description: Trigger Focus modes, HomeKit scenes, and modern macOS App Intents safely through MacFly's allowlist-gated Shortcuts adapter.
order: 5
---

Many modern macOS apps — including **Apple Home (HomeKit)**, **Focus modes**,
and third-party **App Intents** — do not expose an AppleScript dictionary.
Instead of resorting to fragile UI scripting, MacFly integrates with Apple's
official **`shortcuts` CLI** behind a strict allowlist and plan workflow.

## 1. Discover available shortcuts

```bash
macfly shortcuts list
```

By default, **`shortcuts.allow` is empty** — every shortcut is shown as
`Blocked` until you explicitly allow it in `~/.config/macfly/config.json`.

## 2. Allowlist only the shortcuts you trust

Edit `~/.config/macfly/config.json` and add the exact names of the shortcuts
you want MacFly (or your AI agent) to be allowed to propose:

```json
"shortcuts": {
  "allow": [
    "Office Focus",
    "End of Day Lights"
  ]
}
```

Any shortcut not listed in `shortcuts.allow` is rejected immediately with
`SCOPE_DENIED`.

## 3. Run through Plan → Approve → Apply

```json
{
  "summary": "Switch desk into Focus mode",
  "operations": [
    {
      "action": "shortcuts_run",
      "name": "Office Focus"
    }
  ]
}
```

```bash
macfly plan create --input focus.json
macfly plan approve <plan-id>
macfly plan apply <plan-id>
```

## Example use cases

- **Focus & workspace routines:** Create a Shortcut that enables your *Work*
  Focus mode and mutes notifications, then trigger it as part of a MacFly plan.
- **Home automation (HomeKit scenes):** Encapsulate lighting or climate changes
  into a single HomeKit scene inside the Shortcuts app (e.g. *End of Day
  Lights*) and allowlist only that scene — never expose security-critical
  accessories like door locks.
- **Local AI workflows:** Pair MacFly with a local LLM (Ollama, LM Studio, MLX)
  so your home state, calendar, and personal routines stay 100 % on your Mac.
