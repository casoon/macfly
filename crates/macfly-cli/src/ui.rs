use std::io;

use macfly_adapters::executor::ApplyReport;
use macfly_adapters::music::{
    MusicGroupStat, MusicPlaylistsResult, MusicStatsResult, MusicTrackStat, MusicTracksResult,
};
use macfly_core::error::MacflyError;
use macfly_core::model::{
    ContactsAuditReport, DailyBrief, MailOverview, MeetingPrepContext, NotesAuditReport,
    NotesOverview, PersonContext, ShortcutsListResult, WeeklyReviewContext,
};
use macfly_core::plan::{PlanStatus, RiskLevel, StoredPlan};
use macfly_core::plan_text::{Lang, plan_steps, risk_label, undo_label};
use runemark::{
    Align, Cell, ColorMode, Column, Console, DetailLevel, ErrorBlock, Finding, FindingGroup, Group,
    Item, KeyValues, Menu, Metric, NextStep, Outcome, Report, SelectMode, Table, Tone, Verdict,
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
    preset: &str,
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
        .add_metric(Metric::new("Profile", profile))
        .add_metric(
            Metric::new("Preset", preset).with_tone(if preset == "open" {
                Tone::Warning
            } else {
                Tone::Success
            }),
        );

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
        .add_metric(Metric::new(
            "Folders",
            rep.total_folders_scanned.to_string(),
        ))
        .add_metric(Metric::new("Notes", rep.total_notes_scanned.to_string()))
        .add_metric(Metric::new(
            "Empty folders",
            rep.empty_folders.len().to_string(),
        ))
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
        .add_metric(Metric::new(
            "Approved by",
            plan.approved_by.as_deref().unwrap_or("pending"),
        ))
        .add_metric(Metric::new("Hash", &plan.plan_hash));

    let mut ops = FindingGroup::new("Planned operations (full preview)")
        .with_total_count(plan.operations.len());
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

pub fn render_policy(rep: &crate::PolicyReport, color: ColorMode) {
    let console = Console::stdout(color);
    let verdict = if rep.preset == "open" {
        Verdict::Warning
    } else {
        Verdict::Info
    };
    let mut report = Report::new(format!("MacFly Policy · {}", rep.profile), verdict)
        .with_detail_level(DetailLevel::Detailed)
        .add_metric(Metric::new("Preset", rep.preset))
        .add_metric(Metric::new(
            "DLP",
            if rep.dlp_enabled {
                rep.dlp_rules.join(", ")
            } else {
                "off".to_string()
            },
        ))
        .add_metric(Metric::new("Unredacted reads", rep.unredacted_reads));

    let mut about = FindingGroup::new("Sicherheitsstufe");
    about = about.add_finding(Finding::new(Tone::Info, rep.preset_description));
    report = report.add_group(about);

    let mut auto = FindingGroup::new("Läuft automatisch (mit Audit-Journal)");
    if rep.auto_approved_actions.is_empty() {
        auto = auto.add_finding(Finding::new(
            Tone::Info,
            "nichts — jede Änderung braucht deine Freigabe",
        ));
    }
    for a in &rep.auto_approved_actions {
        auto = auto.add_finding(Finding::new(Tone::Success, *a));
    }
    if let Some(max) = rep.max_auto_operations {
        auto = auto.add_finding(Finding::new(
            Tone::Info,
            format!("nur Pläne mit höchstens {max} Operationen"),
        ));
    }
    report = report.add_group(auto);

    let mut manual = FindingGroup::new("Braucht deine Freigabe (macfly plan approve)");
    for a in &rep.approval_required_actions {
        manual = manual.add_finding(Finding::new(Tone::Warning, *a));
    }
    report = report.add_group(manual);

    let mut scope = FindingGroup::new("Sperren & Allowlists");
    scope = scope.add_finding(Finding::new(
        Tone::Info,
        format!(
            "Gesperrte Notes-Ordner: {}",
            list_or_dash(&rep.notes_deny_folders)
        ),
    ));
    if !rep.notes_allow_folders.is_empty() {
        scope = scope.add_finding(Finding::new(
            Tone::Info,
            format!(
                "Nur diese Notes-Ordner: {}",
                rep.notes_allow_folders.join(", ")
            ),
        ));
    }
    if !rep.mail_allow_accounts.is_empty() {
        scope = scope.add_finding(Finding::new(
            Tone::Info,
            format!(
                "Nur diese Mail-Accounts: {}",
                rep.mail_allow_accounts.join(", ")
            ),
        ));
    }
    scope = scope.add_finding(Finding::new(
        Tone::Info,
        format!(
            "Gesperrte Postfächer: {}",
            list_or_dash(&rep.mail_deny_mailboxes)
        ),
    ));
    scope = scope.add_finding(Finding::new(
        Tone::Info,
        format!(
            "Freigegebene Kurzbefehle: {}",
            list_or_dash(&rep.shortcuts_allow)
        ),
    ));
    report = report.add_group(scope);

    let mut never = FindingGroup::new("Nie");
    for n in &rep.never {
        never = never.add_finding(Finding::new(Tone::Info, *n));
    }
    report = report.add_group(never);

    print!("{}", report.render(console));
}

fn list_or_dash(items: &[String]) -> String {
    if items.is_empty() {
        "—".to_string()
    } else {
        items.join(", ")
    }
}

/// Freigabe-Ansicht: der Ablauf in wenigen Schritten, Inhalte zum Prüfen vollständig.
/// Mit `details` zusätzlich alle Einzelheiten und die technischen Angaben.
pub fn render_plan_for_approval(plan: &StoredPlan, color: ColorMode, lang: Lang, details: bool) {
    let console = Console::stdout(color);
    let risk_tone = match plan.risk_level {
        RiskLevel::Low => Tone::Success,
        RiskLevel::Medium => Tone::Warning,
        RiskLevel::High => Tone::Error,
    };
    let title = format!("{} {}", lang.t("Freigabe:", "Approval:"), plan.summary);
    let mut report = Report::new(title, Verdict::ActionRequired)
        .with_detail_level(DetailLevel::Detailed)
        .add_metric(
            Metric::new(lang.t("Risiko", "Risk"), risk_label(plan.risk_level, lang))
                .with_tone(risk_tone),
        )
        .add_metric(Metric::new(
            lang.t("Rückgängig machbar", "Can be undone"),
            undo_label(plan.reversibility, lang),
        ));

    let steps = plan_steps(&plan.operations, lang);
    let mut group = FindingGroup::new(lang.t("Das wird gemacht", "What will happen"))
        .with_total_count(steps.len());
    for (i, step) in steps.iter().enumerate() {
        let mut text = format!("{}. {}", i + 1, step.title);
        for note in &step.notes {
            text.push_str("\n     ");
            text.push_str(note);
        }
        let tone = if step.high_risk {
            Tone::Warning
        } else {
            Tone::Info
        };
        group = group.add_finding(Finding::new(tone, text));
    }
    report = report.add_group(group);

    if details {
        let mut all = FindingGroup::new(lang.t("Alle Einzelheiten", "All details"))
            .with_total_count(plan.operations.len());
        for op in &plan.operations {
            all = all.add_finding(Finding::new(Tone::Info, op.detailed_preview().join("\n")));
        }
        report = report
            .add_group(all)
            .add_metric(Metric::new("Plan", &plan.plan_id))
            .add_metric(Metric::new("Hash", &plan.plan_hash));
    } else {
        report = report.add_next_step(
            NextStep::new(lang.t("Alle Einzelheiten ansehen", "See every detail"))
                .with_command(format!("macfly plan approve {} --details", plan.plan_id)),
        );
    }
    print!("{}", report.render(console));
}

/// Ergebnis-Ansicht: pro Schritt eine Zeile, technische Angaben nur mit `details`.
pub fn render_apply_report(
    rep: &ApplyReport,
    plan: &StoredPlan,
    color: ColorMode,
    lang: Lang,
    details: bool,
) {
    let console = Console::stdout(color);
    let verdict = match rep.status {
        PlanStatus::Applied => Verdict::Passed,
        PlanStatus::PartiallyApplied => Verdict::Warning,
        _ => Verdict::Failed,
    };
    let title = format!("{} {}", lang.t("Erledigt:", "Done:"), plan.summary);
    let mut report = Report::new(title, verdict).with_detail_level(DetailLevel::Detailed);
    if rep.skipped_count > 0 {
        report = report.add_metric(Metric::new(
            lang.t("Schon vorher erledigt", "Already done before"),
            rep.skipped_count.to_string(),
        ));
    }

    let steps = plan_steps(&plan.operations, lang);
    let mut group = FindingGroup::new(lang.t("Was passiert ist", "What happened"))
        .with_total_count(steps.len());
    for step in &steps {
        let mut text = step.title.clone();
        // Ein einzelner Schritt darf seine erste Ergebniszeile als Klartext mitbringen (Fotos-Export).
        if step.ops.len() == 1
            && let Some(msg) = rep.results.get(step.ops.start)
            && let Some((friendly, _)) = msg.split_once('\n')
        {
            text.push_str("\n     ");
            text.push_str(friendly);
        }
        if details {
            for idx in step.ops.clone() {
                if let Some(line) = rep.results.get(idx) {
                    for l in line.lines() {
                        text.push_str("\n     · ");
                        text.push_str(l);
                    }
                }
            }
        }
        group = group.add_finding(Finding::new(Tone::Success, text));
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
        .add_metric(Metric::new(
            "Total contacts",
            rep.total_contacts.to_string(),
        ))
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
        let mut g = FindingGroup::new("Duplicate contact names")
            .with_total_count(rep.duplicate_names.len());
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
            g = g.add_finding(Finding::new(
                Tone::Info,
                format!("{} [id: {}]", c.name, c.id),
            ));
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
        .add_metric(Metric::new(
            "Reminders",
            ctx.open_reminders.len().to_string(),
        ));

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
        .add_metric(Metric::new(
            "Recent notes",
            rev.recent_notes.len().to_string(),
        ))
        .add_metric(Metric::new(
            "Unread mails",
            rev.unread_mails.len().to_string(),
        ))
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
            g = g.add_finding(Finding::new(
                Tone::Muted,
                format!("{} — {}", t.title, t.url),
            ));
        }
        report = report.add_group(g);
    }

    print!("{}", report.render(console));
}

/// One option of an interactive choice: id, label, description.
pub type Choice<'a> = (&'a str, &'a str, &'a str);

/// Interactive single choice (arrow keys, Enter). The cursor starts on the first entry, so list the
/// default first. `Ok(None)` means the menu did not run (no terminal) or was cancelled with Esc.
pub fn choose(
    heading: &str,
    note: &str,
    choices: &[Choice<'_>],
    color: ColorMode,
) -> io::Result<Option<String>> {
    let mut group = Group::new("");
    for (id, label, description) in choices {
        group = group.add_item(Item::new(*id, *label).with_description(*description));
    }
    let menu = Menu::new()
        .with_heading(heading)
        .with_note(note)
        .with_wrapped_descriptions()
        .add_group(group);
    let outcome = menu.run(
        Console::stderr(color),
        SelectMode::Auto,
        io::IsTerminal::is_terminal(&io::stderr()),
    )?;
    Ok(match outcome {
        Outcome::Selected(id) => Some(id),
        Outcome::Hotkey(_) | Outcome::Cancelled | Outcome::Unavailable => None,
    })
}

// ---------- Apple Music ----------

fn tr(lang: Lang, de: &str, en: &str) -> String {
    match lang {
        Lang::De => de.to_string(),
        Lang::En => en.to_string(),
    }
}

fn grouped(n: u64, lang: Lang) -> String {
    let sep = if lang == Lang::De { '.' } else { ',' };
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(sep);
        }
        out.push(c);
    }
    out
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        format!(
            "{}…",
            text.chars().take(max - 1).collect::<String>().trim_end()
        )
    }
}

fn music_section(console: Console, title: String) {
    println!();
    println!("{}", console.paint(Tone::Title, title));
}

fn day(iso: Option<&str>) -> String {
    iso.and_then(|d| d.get(..10)).unwrap_or("—").to_string()
}

fn none_line(console: Console, lang: Lang) {
    println!(
        "{}",
        console.paint(Tone::Muted, tr(lang, "  (keine)", "  (none)"))
    );
}

fn track_table(console: Console, lang: Lang, rows: &[MusicTrackStat], skips: bool) {
    if rows.is_empty() {
        return none_line(console, lang);
    }
    let count_title = if skips {
        tr(lang, "Übersprungen", "Skips")
    } else {
        tr(lang, "Wiedergaben", "Plays")
    };
    let mut t = Table::new()
        .add_column(Column::new().with_title("#").with_align(Align::Right))
        .add_column(
            Column::new()
                .with_title(count_title)
                .with_align(Align::Right),
        )
        .add_column(Column::new().with_title(tr(lang, "Interpret", "Artist")))
        .add_column(Column::new().with_title(tr(lang, "Titel", "Title")))
        .add_column(Column::new().with_title(tr(lang, "Zuletzt", "Last played")));
    for (i, r) in rows.iter().enumerate() {
        let n = if skips {
            r.skipped_count
        } else {
            r.played_count
        };
        t = t.add_row([
            Cell::new((i + 1).to_string()).with_tone(Tone::Muted),
            Cell::new(n.to_string()).with_tone(Tone::Info),
            Cell::new(clip(&r.artist, 28)),
            Cell::new(clip(&r.title, 44)),
            Cell::new(day(r.last_played.as_deref())).with_tone(Tone::Muted),
        ]);
    }
    print!("{}", t.render(console));
}

fn group_table(console: Console, lang: Lang, first: String, rows: &[MusicGroupStat], total: u64) {
    if rows.is_empty() {
        return none_line(console, lang);
    }
    let max = rows.iter().map(|g| g.plays).max().unwrap_or(1).max(1);
    let mut t = Table::new()
        .add_column(Column::new().with_title("#").with_align(Align::Right))
        .add_column(
            Column::new()
                .with_title(tr(lang, "Wiedergaben", "Plays"))
                .with_align(Align::Right),
        )
        .add_column(
            Column::new()
                .with_title(tr(lang, "Anteil", "Share"))
                .with_align(Align::Right),
        )
        .add_column(Column::new())
        .add_column(
            Column::new()
                .with_title(tr(lang, "Titel", "Tracks"))
                .with_align(Align::Right),
        )
        .add_column(Column::new().with_title(first));
    for (i, g) in rows.iter().enumerate() {
        let bar = "▇".repeat((g.plays * 12).div_ceil(max) as usize);
        let pct = g.plays as f64 * 100.0 / total.max(1) as f64;
        t = t.add_row([
            Cell::new((i + 1).to_string()).with_tone(Tone::Muted),
            Cell::new(g.plays.to_string()).with_tone(Tone::Info),
            Cell::new(format!("{pct:.0} %")).with_tone(Tone::Muted),
            Cell::new(bar).with_tone(Tone::Success),
            Cell::new(g.tracks.to_string()).with_tone(Tone::Muted),
            Cell::new(clip(&g.name, 56)),
        ]);
    }
    print!("{}", t.render(console));
}

pub fn render_music_stats(res: &MusicStatsResult, stale_days: u64, color: ColorMode) {
    let console = Console::stdout(color);
    let lang = Lang::detect();
    let s = &res.summary;
    println!(
        "{}",
        console.paint(
            Tone::Title,
            tr(
                lang,
                "Apple Music · Hörstatistik",
                "Apple Music · listening stats"
            )
        )
    );
    println!("{}", console.rule(52));
    let played = s.tracks - s.never_played;
    let kv = KeyValues::new()
        .add(
            tr(lang, "Titel in der Mediathek", "Tracks in library"),
            grouped(s.tracks as u64, lang),
        )
        .add_toned(
            tr(lang, "Wiedergaben", "Plays"),
            Tone::Info,
            format!(
                "{}  (≈ {} h)",
                grouped(s.total_plays, lang),
                s.listened_hours
            ),
        )
        .add(
            tr(lang, "Schon gehört", "Ever played"),
            format!(
                "{}  ({:.0} %)",
                grouped(played as u64, lang),
                played as f64 * 100.0 / s.tracks.max(1) as f64
            ),
        )
        .add(
            tr(lang, "Nie gespielt", "Never played"),
            grouped(s.never_played as u64, lang),
        )
        .add(
            tr(lang, "In den letzten 30 Tagen", "Played in last 30 days"),
            grouped(s.played_last_30_days as u64, lang),
        )
        .add(
            tr(lang, "Übersprungen", "Skips"),
            grouped(s.total_skips, lang),
        );
    print!("{}", kv.render(console));

    music_section(console, tr(lang, "Meistgespielte Titel", "Top tracks"));
    track_table(console, lang, &res.top_tracks, false);
    music_section(
        console,
        tr(lang, "Meistgespielte Interpreten", "Top artists"),
    );
    group_table(
        console,
        lang,
        tr(lang, "Interpret", "Artist"),
        &res.top_artists,
        s.total_plays,
    );
    music_section(console, tr(lang, "Meistgespielte Alben", "Top albums"));
    group_table(
        console,
        lang,
        "Album".to_string(),
        &res.top_albums,
        s.total_plays,
    );
    music_section(console, "Genres".to_string());
    group_table(
        console,
        lang,
        "Genre".to_string(),
        &res.top_genres,
        s.total_plays,
    );
    music_section(
        console,
        tr(lang, "Am häufigsten übersprungen", "Most skipped"),
    );
    track_table(console, lang, &res.most_skipped, true);
    music_section(
        console,
        match lang {
            Lang::De => {
                format!("Wiederentdecken · gehört, aber seit {stale_days}+ Tagen nicht mehr")
            }
            Lang::En => format!("Rediscover · played before, not in {stale_days}+ days"),
        },
    );
    track_table(console, lang, &res.stale, false);
    music_section(
        console,
        tr(
            lang,
            "Nie gespielt · älteste Zugänge zuerst",
            "Never played · oldest additions first",
        ),
    );
    if res.never_played_oldest.is_empty() {
        none_line(console, lang);
    } else {
        let mut t = Table::new()
            .add_column(Column::new().with_title(tr(lang, "Hinzugefügt", "Added")))
            .add_column(Column::new().with_title(tr(lang, "Interpret", "Artist")))
            .add_column(Column::new().with_title(tr(lang, "Titel", "Title")));
        for r in &res.never_played_oldest {
            t = t.add_row([
                Cell::new(day(r.added_at.as_deref())).with_tone(Tone::Muted),
                Cell::new(clip(&r.artist, 28)),
                Cell::new(clip(&r.title, 56)),
            ]);
        }
        print!("{}", t.render(console));
    }
    println!();
    println!(
        "{}",
        console.paint(
            Tone::Muted,
            tr(
                lang,
                "Music führt nur Zähler pro Titel, keinen Hörverlauf. Alle Titel als Tabelle: --csv <datei>",
                "Music keeps counters per track, not a listening history. Every track as a spreadsheet: --csv <file>",
            )
        )
    );
}

pub fn render_music_playlists(res: &MusicPlaylistsResult, color: ColorMode) {
    let console = Console::stdout(color);
    let lang = Lang::detect();
    println!(
        "{}",
        console.paint(
            Tone::Title,
            format!("Apple Music · {} Playlists", res.total_matched)
        )
    );
    let mut t = Table::new()
        .add_column(
            Column::new()
                .with_title(tr(lang, "Titel", "Tracks"))
                .with_align(Align::Right),
        )
        .add_column(Column::new().with_title("Playlist"))
        .add_column(Column::new().with_title(tr(lang, "Art", "Kind")));
    for p in &res.playlists {
        let tone = if p.kind == "playlist" {
            Tone::Muted
        } else {
            Tone::Info
        };
        t = t.add_row([
            Cell::new(p.track_count.to_string()),
            Cell::new(clip(&p.name, 60)),
            Cell::new(p.kind.clone()).with_tone(tone),
        ]);
    }
    print!("{}", t.render(console));
}

pub fn render_music_tracks(res: &MusicTracksResult, color: ColorMode) {
    let console = Console::stdout(color);
    let lang = Lang::detect();
    let title = match lang {
        Lang::De => format!(
            "Apple Music · {} von {} Titeln",
            res.returned, res.total_matched
        ),
        Lang::En => format!(
            "Apple Music · {} of {} tracks",
            res.returned, res.total_matched
        ),
    };
    println!("{}", console.paint(Tone::Title, title));
    let mut t = Table::new()
        .add_column(Column::new().with_title("ID"))
        .add_column(Column::new().with_title(tr(lang, "Interpret", "Artist")))
        .add_column(Column::new().with_title(tr(lang, "Titel", "Title")))
        .add_column(Column::new().with_title("Album"));
    for r in &res.tracks {
        t = t.add_row([
            Cell::new(r.id.clone()).with_tone(Tone::Muted),
            Cell::new(r.artist.clone()),
            Cell::new(r.title.clone()),
            Cell::new(r.album.clone()).with_tone(Tone::Muted),
        ]);
    }
    print!("{}", t.render(console));
}
