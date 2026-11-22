use crate::model::{DailyBrief, PersonContext, WeeklyReviewContext};

/// Erzeugt ein druckfertiges Typst-Dokument (`.typ`) für ein 360°-Personen-/Kunden-Dossier
/// (kompatibel mit `typst compile` und `renderreport`).
pub fn person_context_to_typst(ctx: &PersonContext) -> String {
    let mut out = String::new();
    out.push_str(
        r#"#set page(paper: "a4", margin: (x: 2cm, y: 2cm))
#set text(font: ("Inter", "Helvetica Neue", "Arial"), size: 10pt)
#set par(justify: false, leading: 0.65em)

"#,
    );
    out.push_str(&format!(
        "= MacFly 360° Dossier: {}\n",
        escape_typst(&ctx.query)
    ));
    out.push_str(&format!(
        "#text(fill: rgb(\"#666666\"))[Profile: `{}` · Contacts: {} · Mails: {} · Notes: {} · Events: {} · Reminders: {}]\n\n",
        escape_typst(&ctx.profile),
        ctx.contacts.len(),
        ctx.recent_mails.len(),
        ctx.matching_notes.len(),
        ctx.upcoming_events.len(),
        ctx.open_reminders.len()
    ));

    out.push_str("== Contacts\n");
    if ctx.contacts.is_empty() {
        out.push_str("_No matching contacts._\n\n");
    } else {
        for c in &ctx.contacts {
            let org = c
                .organization
                .as_deref()
                .map(|o| format!(" ({})", escape_typst(o)))
                .unwrap_or_default();
            out.push_str(&format!(
                "- *{}*{}: {} · {}\n",
                escape_typst(&c.name),
                org,
                escape_typst(&c.emails.join(", ")),
                escape_typst(&c.phones.join(", "))
            ));
        }
        out.push('\n');
    }

    out.push_str("== Recent Emails\n");
    if ctx.recent_mails.is_empty() {
        out.push_str("_No matching emails._\n\n");
    } else {
        for m in &ctx.recent_mails {
            let date = m
                .date_received
                .as_deref()
                .and_then(|s| s.get(..10))
                .unwrap_or("—");
            out.push_str(&format!(
                "- `{}` *[{}]* {} — _{}_\n",
                escape_typst(date),
                escape_typst(&m.account),
                escape_typst(&m.subject),
                escape_typst(&m.sender)
            ));
        }
        out.push('\n');
    }

    out.push_str("== Matching Apple Notes\n");
    if ctx.matching_notes.is_empty() {
        out.push_str("_No matching notes._\n\n");
    } else {
        for n in &ctx.matching_notes {
            let date = n
                .modified_at
                .as_deref()
                .and_then(|s| s.get(..10))
                .unwrap_or("—");
            out.push_str(&format!(
                "- `{}` `[{}]` *{}*\n",
                escape_typst(date),
                escape_typst(&n.folder_path),
                escape_typst(&n.title)
            ));
        }
        out.push('\n');
    }

    out.push_str("== Upcoming Events & Open Reminders\n");
    if ctx.upcoming_events.is_empty() && ctx.open_reminders.is_empty() {
        out.push_str("_No upcoming events or open reminders._\n");
    } else {
        for ev in &ctx.upcoming_events {
            let start = ev
                .start_at
                .as_deref()
                .and_then(|s| s.get(..16))
                .unwrap_or("?");
            out.push_str(&format!(
                "- *Event:* `{}` [{}] {}\n",
                escape_typst(start),
                escape_typst(&ev.calendar),
                escape_typst(&ev.title)
            ));
        }
        for r in &ctx.open_reminders {
            out.push_str(&format!(
                "- *Reminder:* [{}] {}\n",
                escape_typst(&r.list),
                escape_typst(&r.title)
            ));
        }
    }

    out
}

pub fn person_context_to_markdown(ctx: &PersonContext) -> String {
    let mut md = format!(
        "---\ntype: \"macfly.dossier.person\"\nquery: {:?}\nprofile: {:?}\n---\n\n# 360° Dossier: {}\n\n",
        ctx.query, ctx.profile, ctx.query
    );
    md.push_str("## Contacts\n");
    for c in &ctx.contacts {
        let org = c
            .organization
            .as_deref()
            .map(|o| format!(" ({o})"))
            .unwrap_or_default();
        md.push_str(&format!(
            "- **{}**{}: {} | {}\n",
            c.name,
            org,
            c.emails.join(", "),
            c.phones.join(", ")
        ));
    }
    md.push_str("\n## Recent Emails\n");
    for m in &ctx.recent_mails {
        let date = m
            .date_received
            .as_deref()
            .and_then(|s| s.get(..10))
            .unwrap_or("—");
        md.push_str(&format!(
            "- `{date}` **[{}]** {} — *{}*\n",
            m.account, m.subject, m.sender
        ));
    }
    md.push_str("\n## Matching Notes\n");
    for n in &ctx.matching_notes {
        let date = n
            .modified_at
            .as_deref()
            .and_then(|s| s.get(..10))
            .unwrap_or("—");
        md.push_str(&format!("- `{date}` `[{}]` **{}**\n", n.folder_path, n.title));
    }
    md.push_str("\n## Events & Reminders\n");
    for ev in &ctx.upcoming_events {
        let start = ev
            .start_at
            .as_deref()
            .and_then(|s| s.get(..16))
            .unwrap_or("?");
        md.push_str(&format!("- **Event:** `{start}` [{}] {}\n", ev.calendar, ev.title));
    }
    for r in &ctx.open_reminders {
        md.push_str(&format!("- [ ] `[{}]` {}\n", r.list, r.title));
    }
    md
}

pub fn daily_brief_to_typst(brief: &DailyBrief) -> String {
    let mut out = String::new();
    out.push_str(
        r#"#set page(paper: "a4", margin: (x: 2cm, y: 2cm))
#set text(font: ("Inter", "Helvetica Neue", "Arial"), size: 10pt)

"#,
    );
    out.push_str(&format!(
        "= MacFly Daily Brief ({})\n\n",
        escape_typst(&brief.profile)
    ));
    out.push_str(&format!(
        "== Upcoming Events ({})\n",
        brief.upcoming_events.len()
    ));
    for ev in &brief.upcoming_events {
        let start = ev
            .start_at
            .as_deref()
            .and_then(|s| s.get(..16))
            .unwrap_or("?");
        out.push_str(&format!(
            "- `{}` *[{}]* {}\n",
            escape_typst(start),
            escape_typst(&ev.calendar),
            escape_typst(&ev.title)
        ));
    }
    out.push_str(&format!(
        "\n== Open Reminders ({})\n",
        brief.open_reminders.len()
    ));
    for r in &brief.open_reminders {
        out.push_str(&format!(
            "- *[{}]* {}\n",
            escape_typst(&r.list),
            escape_typst(&r.title)
        ));
    }
    out.push_str(&format!(
        "\n== Unread Emails ({})\n",
        brief.unread_mails.len()
    ));
    for m in &brief.unread_mails {
        out.push_str(&format!(
            "- *[{}]* {} — _{}_\n",
            escape_typst(&m.account),
            escape_typst(&m.subject),
            escape_typst(&m.sender)
        ));
    }
    out.push_str(&format!(
        "\n== Recent Notes ({})\n",
        brief.recent_notes.len()
    ));
    for n in &brief.recent_notes {
        out.push_str(&format!(
            "- `[{}]` *{}*\n",
            escape_typst(&n.folder_path),
            escape_typst(&n.title)
        ));
    }
    out
}

pub fn weekly_review_to_typst(rev: &WeeklyReviewContext) -> String {
    let mut out = String::new();
    out.push_str(
        r#"#set page(paper: "a4", margin: (x: 2cm, y: 2cm))
#set text(font: ("Inter", "Helvetica Neue", "Arial"), size: 10pt)

"#,
    );
    out.push_str(&format!(
        "= MacFly Weekly Review ({})\n\n",
        escape_typst(&rev.profile)
    ));
    out.push_str(&format!(
        "== Modified Notes (last 7 days: {})\n",
        rev.recent_notes.len()
    ));
    for n in &rev.recent_notes {
        out.push_str(&format!(
            "- `[{}]` *{}*\n",
            escape_typst(&n.folder_path),
            escape_typst(&n.title)
        ));
    }
    out.push_str(&format!(
        "\n== Overdue Reminders ({})\n",
        rev.overdue_reminders.len()
    ));
    for r in &rev.overdue_reminders {
        out.push_str(&format!(
            "- *[{}]* {}\n",
            escape_typst(&r.list),
            escape_typst(&r.title)
        ));
    }
    out.push_str(&format!(
        "\n== Unread Emails ({})\n",
        rev.unread_mails.len()
    ));
    for m in &rev.unread_mails {
        out.push_str(&format!(
            "- *[{}]* {} — _{}_\n",
            escape_typst(&m.account),
            escape_typst(&m.subject),
            escape_typst(&m.sender)
        ));
    }
    out.push_str(&format!(
        "\n== Open Safari Tabs ({})\n",
        rev.open_safari_tabs.len()
    ));
    for t in &rev.open_safari_tabs {
        out.push_str(&format!(
            "- *{}* — `{}`\n",
            escape_typst(&t.title),
            escape_typst(&t.url)
        ));
    }
    out
}

fn escape_typst(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('#', "\\#")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('`', "\\`")
        .replace('$', "\\$")
        .replace('@', "\\@")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_person_dossier_typst_and_md() {
        let ctx = PersonContext {
            schema_version: "1.0".to_string(),
            profile: "default".to_string(),
            query: "Acme #1".to_string(),
            generated_at_unix: 0,
            contacts: Vec::new(),
            recent_mails: Vec::new(),
            matching_notes: Vec::new(),
            upcoming_events: Vec::new(),
            open_reminders: Vec::new(),
            warnings: Vec::new(),
        };
        let typ = person_context_to_typst(&ctx);
        assert!(typ.contains("MacFly 360° Dossier: Acme \\#1"));
        let md = person_context_to_markdown(&ctx);
        assert!(md.contains("# 360° Dossier: Acme #1"));
    }
}
