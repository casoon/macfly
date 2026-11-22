use std::io;

use macfly_adapters::executor::ApplyReport;
use macfly_core::error::MacflyError;
use macfly_core::model::{
    ContactsAuditReport, DailyBrief, MailOverview, MeetingPrepContext, NotesAuditReport,
    NotesOverview, PersonContext, ShortcutsListResult, WeeklyReviewContext,
};
use macfly_core::plan::{PlanStatus, RiskLevel, StoredPlan};
use runemark::{
    ColorMode, Console, DetailLevel, ErrorBlock, Finding, FindingGroup, Metric, NextStep, Report,
    Tone, Verdict,
};

pub fn render_error(err: &MacflyError, color: ColorMode) {
    let console = Console::stderr(color);
    let mut block = ErrorBlock::new(format!("[{}] {}", err.code(), err));
    if let Some(hint) = err.hint() {
        block = block.with_remedy(hint);
    }
    if err.code() == "TCC_PERMISSION_DENIED" {
        block = block.add_command("macfly doctor");
    }
    let mut stderr = io::stderr().lock();
    let _ = block.write_to(console, &mut stderr);
}

#[allow(clippy::too_many_arguments)]
pub fn render_doctor(
    os: &str,
    osascript_ok: bool,
    config_path: &str,
    config_exists: bool,
    profile: &str,
    notes_access: &str,
    notes_accounts: Option<usize>,
    notes_total: Option<usize>,
    mail_access: &str,
    mail_accounts: Option<usize>,
    mail_unread: Option<usize>,
    mail_sigs: Option<usize>,
    hints: &[String],
    color: ColorMode,
) {
    let console = Console::stdout(color);
    let all_ok = osascript_ok && notes_access == "ok" && mail_access == "ok";
    let verdict = if !osascript_ok {
        Verdict::Failed
    } else if all_ok {
        Verdict::Passed
    } else {
        Verdict::Warning
    };

    let mut report = Report::new("MacFly Doctor", verdict)
        .add_metric(Metric::new("OS", os))
        .add_metric(
            Metric::new("osascript", if osascript_ok { "OK" } else { "missing" }).with_tone(
                if osascript_ok {
                    Tone::Success
                } else {
                    Tone::Error
                },
            ),
        )
        .add_metric(Metric::new(
            "Config",
            format!(
                "{} ({})",
                config_path,
                if config_exists { "loaded" } else { "defaults" }
            ),
        ))
        .add_metric(Metric::new("Profile", profile));

    let mut adapters = FindingGroup::new("macOS Adapters");
    if notes_access == "ok" {
        adapters = adapters.add_finding(Finding::new(
            Tone::Success,
            format!(
                "Apple Notes: ok ({} accounts, {} notes)",
                notes_accounts.unwrap_or(0),
                notes_total.unwrap_or(0)
            ),
        ));
    } else {
        adapters = adapters.add_finding(Finding::new(
            Tone::Warning,
            format!("Apple Notes: {notes_access}"),
        ));
    }

    if mail_access == "ok" {
        adapters = adapters.add_finding(Finding::new(
            Tone::Success,
            format!(
                "Apple Mail: ok ({} accounts, {} unread, {} signatures)",
                mail_accounts.unwrap_or(0),
                mail_unread.unwrap_or(0),
                mail_sigs.unwrap_or(0)
            ),
        ));
    } else {
        adapters = adapters.add_finding(Finding::new(
            Tone::Warning,
            format!("Apple Mail: {mail_access}"),
        ));
    }

    report = report.add_group(adapters);

    if !hints.is_empty() {
        let mut hint_group = FindingGroup::new("Hints");
        for h in hints {
            hint_group = hint_group.add_finding(Finding::new(Tone::Info, h.clone()));
        }
        report = report.add_group(hint_group);
    }

    if !config_exists {
        report = report.add_next_step(
            NextStep::new("Create central configuration file").with_command("macfly config init"),
        );
    }

    print!("{}", report.render(console));
}

pub fn render_notes_overview(ov: &NotesOverview, color: ColorMode) {
    let console = Console::stdout(color);
    println!(
        "{}",
        console.paint(
            Tone::Title,
            format!(
                "Apple Notes Overview (profile: {} | {} accounts | {} folders | {} notes)",
                ov.profile, ov.total_accounts, ov.total_folders, ov.total_notes
            )
        )
    );
    if ov.hidden_by_policy_folders > 0 {
        println!(
            "{}",
            console.paint(
                Tone::Muted,
                format!(
                    "  {} folder(s) hidden by profile policy",
                    ov.hidden_by_policy_folders
                )
            )
        );
    }
    println!();
    for f in &ov.folders {
        let line = format!("  {:>4}  {:<42} ({})", f.note_count, f.path, f.account);
        if f.allowed_by_policy {
            println!("{line}");
        } else {
            println!(
                "{} {}",
                console.paint(Tone::Muted, &line),
                console.paint(Tone::Warning, "[locked]")
            );
        }
    }
}

pub fn render_notes_audit(rep: &NotesAuditReport, detailed: bool, color: ColorMode) {
    let console = Console::stdout(color);
    let has_findings = !rep.empty_folders.is_empty()
        || !rep.duplicate_folder_names.is_empty()
        || !rep.duplicate_titles.is_empty();
    let verdict = if has_findings {
        Verdict::Warning
    } else {
        Verdict::Passed
    };

    let mut report = Report::new("Apple Notes Audit", verdict)
        .with_detail_level(if detailed {
            DetailLevel::Detailed
        } else {
            DetailLevel::Compact
        })
        .with_details_hint(if detailed {
            None
        } else {
            Some("Run `macfly notes audit --details` to list all items.".to_string())
        })
        .add_metric(Metric::new("Profile", &rep.profile))
        .add_metric(Metric::new("Folders", rep.total_folders_scanned.to_string()))
        .add_metric(Metric::new("Notes", rep.total_notes_scanned.to_string()))
        .add_metric(Metric::new("Empty folders", rep.empty_folders.len().to_string()))
        .add_metric(Metric::new(
            "Single-note folders",
            rep.single_note_folders.len().to_string(),
        ))
        .add_metric(Metric::new(
            "Duplicate titles",
            rep.duplicate_titles.len().to_string(),
        ));

    if !rep.empty_folders.is_empty() {
        let mut g = FindingGroup::new("Empty folders").with_total_count(rep.empty_folders.len());
        for f in &rep.empty_folders {
            g = g.add_finding(Finding::new(
                Tone::Warning,
                format!("{} ({}) [id: {}]", f.path, f.account, f.id),
            ));
        }
        report = report.add_group(g);
    }

    if !rep.single_note_folders.is_empty() {
        let mut g = FindingGroup::new("Single-note folders")
            .with_advisory(true)
            .with_total_count(rep.single_note_folders.len());
        for f in &rep.single_note_folders {
            g = g.add_finding(Finding::new(
                Tone::Info,
                format!("{} ({})", f.path, f.account),
            ));
        }
        report = report.add_group(g);
    }

    if !rep.duplicate_folder_names.is_empty() {
        let mut g = FindingGroup::new("Duplicate folder names")
            .with_total_count(rep.duplicate_folder_names.len());
        for (name, paths) in &rep.duplicate_folder_names {
            g = g.add_finding(Finding::new(
                Tone::Warning,
                format!("'{}': {}", name, paths.join(" | ")),
            ));
        }
        report = report.add_group(g);
    }

    if !rep.duplicate_titles.is_empty() {
        let mut g = FindingGroup::new("Potential duplicate titles")
            .with_advisory(true)
            .with_total_count(rep.duplicate_titles.len());
        for dup in &rep.duplicate_titles {
            let folders: Vec<&str> = dup.notes.iter().map(|n| n.folder_path.as_str()).collect();
            g = g.add_finding(Finding::new(
                Tone::Info,
                format!(
                    "'{}' ({}x): {}",
                    dup.normalized_title,
                    dup.count,
                    folders.join(", ")
                ),
            ));
        }
        report = report.add_group(g);
    }

    if !rep.oldest_unmodified_notes.is_empty() {
        let mut g = FindingGroup::new("Oldest unmodified notes")
            .with_advisory(true)
            .with_total_count(rep.oldest_unmodified_notes.len());
        for n in &rep.oldest_unmodified_notes {
            let date = n
                .modified_at
                .as_deref()
                .and_then(|s| s.get(..10))
                .unwrap_or("?");
            g = g.add_finding(Finding::new(
                Tone::Muted,
                format!("{date}  [{}]  {}", n.folder_path, n.title),
            ));
        }
        report = report.add_group(g);
    }

    print!("{}", report.render(console));
}

pub fn render_mail_overview(ov: &MailOverview, color: ColorMode) {
    let console = Console::stdout(color);
    println!(
        "{}",
        console.paint(
            Tone::Title,
            format!(
                "Apple Mail Overview (profile: {} | {} accounts | {} mailboxes | {} unread)",
                ov.profile, ov.total_accounts, ov.total_mailboxes, ov.total_unread
            )
        )
    );
    println!("\n{}", console.paint(Tone::Info, "Accounts:"));
    for a in &ov.accounts {
        println!(
            "  • {} ({}) [{} mailboxes]",
            a.name,
            a.email_addresses.join(", "),
            a.mailbox_count
        );
    }
    println!(
        "\n{}",
        console.paint(
            Tone::Info,
            format!("Signatures from Apple Mail ({}):", ov.signatures.len())
        )
    );
    for s in &ov.signatures {
        println!("  • '{}' ({} chars)", s.name, s.char_count);
    }
}

pub fn render_plan(plan: &StoredPlan, color: ColorMode) {
    let console = Console::stdout(color);
    let verdict = match plan.status {
        PlanStatus::PendingApproval => Verdict::ActionRequired,
        PlanStatus::Approved => Verdict::Info,
        PlanStatus::PartiallyApplied => Verdict::Warning,
        PlanStatus::Applied => Verdict::Passed,
        PlanStatus::Failed => Verdict::Failed,
    };
    let risk_tone = match plan.risk_level {
        RiskLevel::Low => Tone::Success,
        RiskLevel::Medium => Tone::Warning,
        RiskLevel::High => Tone::Error,
    };

    let mut report = Report::new(format!("Change Plan {}", plan.plan_id), verdict)
        .with_detail_level(DetailLevel::Detailed)
        .add_metric(Metric::new("Profile", &plan.profile))
        .add_metric(Metric::new("Status", format!("{:?}", plan.status)))
        .add_metric(Metric::new("Risk", format!("{:?}", plan.risk_level)).with_tone(risk_tone))
        .add_metric(Metric::new(
            "Reversibility",
            format!("{:?}", plan.reversibility),
        ))
        .add_metric(Metric::new("Approval code", &plan.approval_code).with_tone(Tone::Warning))
        .add_metric(Metric::new("Hash", &plan.plan_hash));

    let mut ops = FindingGroup::new("Planned operations (full preview)").with_total_count(plan.operations.len());
    for op in &plan.operations {
        let tone = match op.risk_level() {
            RiskLevel::High => Tone::Warning,
            _ => Tone::Info,
        };
        ops = ops.add_finding(Finding::new(tone, op.detailed_preview().join("\n")));
    }
    report = report.add_group(ops);

    match plan.status {
        PlanStatus::PendingApproval => {
            report = report.add_next_step(
                NextStep::new("Review and approve interactively")
                    .with_command(format!("macfly plan approve {}", plan.plan_id)),
            );
        }
        PlanStatus::Approved => {
            report = report.add_next_step(
                NextStep::new("Execute approved plan")
                    .with_command(format!("macfly plan apply {}", plan.plan_id)),
            );
        }
        PlanStatus::PartiallyApplied => {
            report = report.add_next_step(
                NextStep::new("Resume remaining operations")
                    .with_command(format!("macfly plan apply {} --resume", plan.plan_id)),
            );
        }
        _ => {}
    }

    print!("{}", report.render(console));
}

pub fn render_apply_report(rep: &ApplyReport, color: ColorMode) {
    let console = Console::stdout(color);
    let verdict = match rep.status {
        PlanStatus::Applied => Verdict::Passed,
        PlanStatus::PartiallyApplied => Verdict::Warning,
        _ => Verdict::Failed,
    };
    let mut report = Report::new(format!("Plan Applied: {}", rep.plan_id), verdict)
        .with_detail_level(DetailLevel::Detailed)
        .add_metric(Metric::new("Succeeded", format!("{}/{}", rep.succeeded_count, rep.total_ops)))
        .add_metric(Metric::new("Skipped (resumed)", rep.skipped_count.to_string()));
    let mut group = FindingGroup::new("Results").with_total_count(rep.results.len());
    for line in &rep.results {
        group = group.add_finding(Finding::new(Tone::Success, line.clone()));
    }
    report = report.add_group(group);
    print!("{}", report.render(console));
}

pub fn render_daily_brief(brief: &DailyBrief, color: ColorMode) {
    let console = Console::stdout(color);
    let verdict = if brief.warnings.is_empty() {
        Verdict::Info
    } else {
        Verdict::Warning
    };
    let mut report = Report::new(format!("MacFly Daily Brief ({})", brief.profile), verdict)
        .with_detail_level(DetailLevel::Detailed)
        .add_metric(Metric::new(
            "Events (48h)",
            brief.upcoming_events.len().to_string(),
        ))
        .add_metric(Metric::new(
            "Open reminders",
            brief.open_reminders.len().to_string(),
        ))
        .add_metric(Metric::new(
            "Unread mails",
            brief.unread_mail_count.to_string(),
        ))
        .add_metric(Metric::new(
            "Recent notes",
            brief.recent_notes.len().to_string(),
        ));

    if !brief.upcoming_events.is_empty() {
        let mut g = FindingGroup::new("Upcoming events (48h)");
        for ev in &brief.upcoming_events {
            let start = ev
                .start_at
                .as_deref()
                .and_then(|s| s.get(..16))
                .unwrap_or("?");
            g = g.add_finding(Finding::new(
                Tone::Info,
                format!("{start} [{}] {}", ev.calendar, ev.title),
            ));
        }
        report = report.add_group(g);
    }

    if !brief.open_reminders.is_empty() {
        let mut g = FindingGroup::new("Open reminders");
        for r in &brief.open_reminders {
            let tone = if r.overdue { Tone::Warning } else { Tone::Info };
            g = g.add_finding(Finding::new(tone, format!("[{}] {}", r.list, r.title)));
        }
        report = report.add_group(g);
    }

    if !brief.unread_mails.is_empty() {
        let mut g = FindingGroup::new("Unread mails");
        for m in &brief.unread_mails {
            g = g.add_finding(Finding::new(
                Tone::Info,
                format!("[{}] {} — {}", m.account, m.subject, m.sender),
            ));
        }
        report = report.add_group(g);
    }

    if !brief.recent_notes.is_empty() {
        let mut g = FindingGroup::new("Recently modified notes");
        for n in &brief.recent_notes {
            let date = n
                .modified_at
                .as_deref()
                .and_then(|s| s.get(..10))
                .unwrap_or("?");
            g = g.add_finding(Finding::new(
                Tone::Muted,
                format!("{date} [{}] {}", n.folder_path, n.title),
            ));
        }
        report = report.add_group(g);
    }

    if !brief.warnings.is_empty() {
        let mut g = FindingGroup::new("Warnings");
        for w in &brief.warnings {
            g = g.add_finding(Finding::new(Tone::Warning, w.clone()));
        }
        report = report.add_group(g);
    }

    print!("{}", report.render(console));
}

pub fn render_shortcuts_list(res: &ShortcutsListResult, color: ColorMode) {
    let console = Console::stdout(color);
    let verdict = if res.allowed_count > 0 {
        Verdict::Passed
    } else {
        Verdict::Info
    };

    let mut report = Report::new("Apple Shortcuts", verdict)
        .with_detail_level(DetailLevel::Detailed)
        .add_metric(Metric::new("Profile", &res.profile))
        .add_metric(Metric::new("Total", res.total.to_string()))
        .add_metric(
            Metric::new("Allowed", res.allowed_count.to_string()).with_tone(
                if res.allowed_count > 0 {
                    Tone::Success
                } else {
                    Tone::Muted
                },
            ),
        );

    let allowed: Vec<&str> = res
        .shortcuts
        .iter()
        .filter(|s| s.allowed_by_policy)
        .map(|s| s.name.as_str())
        .collect();
    if !allowed.is_empty() {
        let mut g = FindingGroup::new("Allowed by profile").with_total_count(allowed.len());
        for name in allowed {
            g = g.add_finding(Finding::new(Tone::Success, name));
        }
        report = report.add_group(g);
    }

    let blocked: Vec<&str> = res
        .shortcuts
        .iter()
        .filter(|s| !s.allowed_by_policy)
        .map(|s| s.name.as_str())
        .collect();
    if !blocked.is_empty() {
        let mut g = FindingGroup::new("Blocked (not in shortcuts.allow)")
            .with_advisory(true)
            .with_total_count(blocked.len());
        for name in blocked.iter().take(20) {
            g = g.add_finding(Finding::new(Tone::Muted, *name));
        }
        report = report.add_group(g);
    }

    if res.allowed_count == 0 {
        report = report.add_next_step(
            NextStep::new("Add allowed shortcut names to `shortcuts.allow` in config.json")
                .with_command("macfly config path"),
        );
    }

    print!("{}", report.render(console));
}

pub fn render_contacts_audit(rep: &ContactsAuditReport, detailed: bool, color: ColorMode) {
    let console = Console::stdout(color);
    let has_findings = !rep.duplicate_names.is_empty()
        || !rep.duplicate_emails.is_empty()
        || !rep.empty_contacts.is_empty();
    let verdict = if has_findings {
        Verdict::Warning
    } else {
        Verdict::Passed
    };

    let mut report = Report::new("Apple Contacts Audit", verdict)
        .with_detail_level(if detailed {
            DetailLevel::Detailed
        } else {
            DetailLevel::Compact
        })
        .with_details_hint(if detailed {
            None
        } else {
            Some("Run `macfly contacts audit --details` to list all items.".to_string())
        })
        .add_metric(Metric::new("Total contacts", rep.total_contacts.to_string()))
        .add_metric(Metric::new(
            "Duplicate names",
            rep.duplicate_names.len().to_string(),
        ))
        .add_metric(Metric::new(
            "Duplicate emails",
            rep.duplicate_emails.len().to_string(),
        ))
        .add_metric(Metric::new(
            "Without email & phone",
            rep.empty_contacts.len().to_string(),
        ));

    if !rep.duplicate_names.is_empty() {
        let mut g =
            FindingGroup::new("Duplicate contact names").with_total_count(rep.duplicate_names.len());
        for (name, count) in &rep.duplicate_names {
            g = g.add_finding(Finding::new(Tone::Warning, format!("'{name}' ({count}x)")));
        }
        report = report.add_group(g);
    }

    if !rep.duplicate_emails.is_empty() {
        let mut g = FindingGroup::new("Shared/duplicate email addresses")
            .with_total_count(rep.duplicate_emails.len());
        for (email, names) in &rep.duplicate_emails {
            g = g.add_finding(Finding::new(
                Tone::Warning,
                format!("{email}: {}", names.join(", ")),
            ));
        }
        report = report.add_group(g);
    }

    if !rep.empty_contacts.is_empty() {
        let mut g = FindingGroup::new("Contacts without email or phone")
            .with_advisory(true)
            .with_total_count(rep.empty_contacts.len());
        for c in &rep.empty_contacts {
            g = g.add_finding(Finding::new(Tone::Info, format!("{} [id: {}]", c.name, c.id)));
        }
        report = report.add_group(g);
    }

    print!("{}", report.render(console));
}

pub fn render_person_context(ctx: &PersonContext, color: ColorMode) {
    let console = Console::stdout(color);
    let verdict = if ctx.warnings.is_empty() {
        Verdict::Info
    } else {
        Verdict::Warning
    };
    let mut report = Report::new(format!("MacFly 360° Dossier: {}", ctx.query), verdict)
        .with_detail_level(DetailLevel::Detailed)
        .add_metric(Metric::new("Profile", &ctx.profile))
        .add_metric(Metric::new("Contacts", ctx.contacts.len().to_string()))
        .add_metric(Metric::new("Mails", ctx.recent_mails.len().to_string()))
        .add_metric(Metric::new("Notes", ctx.matching_notes.len().to_string()))
        .add_metric(Metric::new("Events", ctx.upcoming_events.len().to_string()))
        .add_metric(Metric::new("Reminders", ctx.open_reminders.len().to_string()));

    if !ctx.contacts.is_empty() {
        let mut g = FindingGroup::new("Contacts").with_total_count(ctx.contacts.len());
        for c in &ctx.contacts {
            let org = c
                .organization
                .as_deref()
                .map(|o| format!(" ({o})"))
                .unwrap_or_default();
            g = g.add_finding(Finding::new(
                Tone::Success,
                format!(
                    "{}{} · {} · {}",
                    c.name,
                    org,
                    c.emails.join(", "),
                    c.phones.join(", ")
                ),
            ));
        }
        report = report.add_group(g);
    }

    if !ctx.recent_mails.is_empty() {
        let mut g = FindingGroup::new("Recent emails").with_total_count(ctx.recent_mails.len());
        for m in &ctx.recent_mails {
            let date = m
                .date_received
                .as_deref()
                .and_then(|s| s.get(..10))
                .unwrap_or("—");
            g = g.add_finding(Finding::new(
                Tone::Info,
                format!("{date} [{}] {} — {}", m.account, m.subject, m.sender),
            ));
        }
        report = report.add_group(g);
    }

    if !ctx.matching_notes.is_empty() {
        let mut g =
            FindingGroup::new("Matching Apple Notes").with_total_count(ctx.matching_notes.len());
        for n in &ctx.matching_notes {
            let date = n
                .modified_at
                .as_deref()
                .and_then(|s| s.get(..10))
                .unwrap_or("—");
            g = g.add_finding(Finding::new(
                Tone::Info,
                format!("{date} [{}] {}", n.folder_path, n.title),
            ));
        }
        report = report.add_group(g);
    }

    if !ctx.upcoming_events.is_empty() || !ctx.open_reminders.is_empty() {
        let mut g = FindingGroup::new("Events & reminders");
        for ev in &ctx.upcoming_events {
            let start = ev
                .start_at
                .as_deref()
                .and_then(|s| s.get(..16))
                .unwrap_or("?");
            g = g.add_finding(Finding::new(
                Tone::Info,
                format!("Event: {start} [{}] {}", ev.calendar, ev.title),
            ));
        }
        for r in &ctx.open_reminders {
            g = g.add_finding(Finding::new(
                Tone::Warning,
                format!("Reminder: [{}] {}", r.list, r.title),
            ));
        }
        report = report.add_group(g);
    }

    print!("{}", report.render(console));
}

pub fn render_meeting_prep(ctx: &MeetingPrepContext, color: ColorMode) {
    let console = Console::stdout(color);
    let mut report = Report::new(format!("Meeting Prep: {}", ctx.query), Verdict::Info)
        .with_detail_level(DetailLevel::Detailed)
        .add_metric(Metric::new("Profile", &ctx.profile))
        .add_metric(Metric::new(
            "Related contacts",
            ctx.related_contacts.len().to_string(),
        ))
        .add_metric(Metric::new(
            "Related mails",
            ctx.related_mails.len().to_string(),
        ))
        .add_metric(Metric::new(
            "Related notes",
            ctx.related_notes.len().to_string(),
        ));

    if let Some(ev) = &ctx.target_event {
        let start = ev
            .start_at
            .as_deref()
            .and_then(|s| s.get(..16))
            .unwrap_or("?");
        let g = FindingGroup::new("Target calendar event").add_finding(Finding::new(
            Tone::Success,
            format!("{start} [{}] {}", ev.calendar, ev.title),
        ));
        report = report.add_group(g);
    }

    if !ctx.related_mails.is_empty() {
        let mut g = FindingGroup::new("Related emails").with_total_count(ctx.related_mails.len());
        for m in &ctx.related_mails {
            g = g.add_finding(Finding::new(
                Tone::Info,
                format!("[{}] {} — {}", m.account, m.subject, m.sender),
            ));
        }
        report = report.add_group(g);
    }

    if !ctx.related_notes.is_empty() {
        let mut g = FindingGroup::new("Related notes").with_total_count(ctx.related_notes.len());
        for n in &ctx.related_notes {
            g = g.add_finding(Finding::new(
                Tone::Info,
                format!("[{}] {}", n.folder_path, n.title),
            ));
        }
        report = report.add_group(g);
    }

    print!("{}", report.render(console));
}

pub fn render_weekly_review(rev: &WeeklyReviewContext, color: ColorMode) {
    let console = Console::stdout(color);
    let verdict = if rev.overdue_reminders.is_empty() {
        Verdict::Info
    } else {
        Verdict::Warning
    };
    let mut report = Report::new(format!("MacFly Weekly Review ({})", rev.profile), verdict)
        .with_detail_level(DetailLevel::Detailed)
        .add_metric(Metric::new("Recent notes", rev.recent_notes.len().to_string()))
        .add_metric(Metric::new("Unread mails", rev.unread_mails.len().to_string()))
        .add_metric(
            Metric::new("Overdue reminders", rev.overdue_reminders.len().to_string()).with_tone(
                if rev.overdue_reminders.is_empty() {
                    Tone::Success
                } else {
                    Tone::Warning
                },
            ),
        )
        .add_metric(Metric::new(
            "Events (7d)",
            rev.upcoming_events.len().to_string(),
        ))
        .add_metric(Metric::new(
            "Safari tabs",
            rev.open_safari_tabs.len().to_string(),
        ));

    if !rev.overdue_reminders.is_empty() {
        let mut g =
            FindingGroup::new("Overdue reminders").with_total_count(rev.overdue_reminders.len());
        for r in &rev.overdue_reminders {
            g = g.add_finding(Finding::new(
                Tone::Warning,
                format!("[{}] {}", r.list, r.title),
            ));
        }
        report = report.add_group(g);
    }

    if !rev.unread_mails.is_empty() {
        let mut g = FindingGroup::new("Unread emails").with_total_count(rev.unread_mails.len());
        for m in rev.unread_mails.iter().take(10) {
            g = g.add_finding(Finding::new(
                Tone::Info,
                format!("[{}] {} — {}", m.account, m.subject, m.sender),
            ));
        }
        report = report.add_group(g);
    }

    if !rev.recent_notes.is_empty() {
        let mut g =
            FindingGroup::new("Recently modified notes").with_total_count(rev.recent_notes.len());
        for n in rev.recent_notes.iter().take(10) {
            let date = n
                .modified_at
                .as_deref()
                .and_then(|s| s.get(..10))
                .unwrap_or("—");
            g = g.add_finding(Finding::new(
                Tone::Muted,
                format!("{date} [{}] {}", n.folder_path, n.title),
            ));
        }
        report = report.add_group(g);
    }

    if !rev.open_safari_tabs.is_empty() {
        let mut g =
            FindingGroup::new("Open Safari tabs").with_total_count(rev.open_safari_tabs.len());
        for t in rev.open_safari_tabs.iter().take(10) {
            g = g.add_finding(Finding::new(Tone::Muted, format!("{} — {}", t.title, t.url)));
        }
        report = report.add_group(g);
    }

    print!("{}", report.render(console));
}
