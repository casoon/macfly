use std::io;

use macfly_adapters::executor::ApplyReport;
use macfly_core::error::MacflyError;
use macfly_core::model::{DailyBrief, MailOverview, NotesAuditReport, NotesOverview};
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
            &format!(
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
                &format!(
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
            &format!(
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
            &format!("Signatures from Apple Mail ({}):", ov.signatures.len())
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

    let mut ops = FindingGroup::new("Planned operations").with_total_count(plan.operations.len());
    for line in &plan.preview_lines {
        ops = ops.add_finding(Finding::new(Tone::Info, line.clone()));
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
        _ => {}
    }

    print!("{}", report.render(console));
}

pub fn render_apply_report(rep: &ApplyReport, color: ColorMode) {
    let console = Console::stdout(color);
    let verdict = if rep.status == PlanStatus::Applied {
        Verdict::Passed
    } else {
        Verdict::Failed
    };
    let mut report = Report::new(format!("Plan Applied: {}", rep.plan_id), verdict)
        .with_detail_level(DetailLevel::Detailed);
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
