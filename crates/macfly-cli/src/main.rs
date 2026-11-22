mod mcp;
mod ui;

use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use clap::{Parser, Subcommand, ValueEnum};
use macfly_adapters::calendar::{CalendarAdapter, RemindersAdapter};
use macfly_adapters::executor::apply_approved_plan;
use macfly_adapters::mail::{ListMailQuery, MailAdapter};
use macfly_adapters::notes::{ListNotesQuery, NotesAdapter};
use macfly_adapters::shortcuts::ShortcutsAdapter;
use macfly_core::config::Config;
use macfly_core::error::MacflyError;
use macfly_core::journal::AuditJournal;
use macfly_core::model::DailyBrief;
use macfly_core::plan::{PlanProposal, PlanStore, now_unix};
use runemark::ColorMode;
use serde::Serialize;

#[derive(Parser)]
#[command(
    name = "macfly",
    version,
    about = "MacFly — sichere, schnelle macOS-Automation & Data Firewall in Rust"
)]
struct Cli {
    /// Ausgabeformat (pretty für Terminal, json für Skripte, agent kompakt für KI)
    #[arg(long, global = true, default_value = "pretty")]
    format: OutputFormat,

    /// Farbausgabe im Pretty-Modus (auto respektiert TTY & NO_COLOR)
    #[arg(long, global = true, default_value = "auto")]
    color: ColorFlag,

    /// Profilname aus der Config (Default: `default_profile` aus config.json)
    #[arg(long, global = true)]
    profile: Option<String>,

    /// Alternativer Pfad zur config.json
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ColorFlag {
    Auto,
    Always,
    Never,
}

impl From<ColorFlag> for ColorMode {
    fn from(value: ColorFlag) -> Self {
        match value {
            ColorFlag::Auto => ColorMode::Auto,
            ColorFlag::Always => ColorMode::Always,
            ColorFlag::Never => ColorMode::Never,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Pretty,
    Json,
    Agent,
}

#[derive(Subcommand)]
enum Commands {
    /// Prüft macOS-Umgebung, Config und Zugriffsrechte für Notes & Mail
    Doctor,
    /// Zentrale Konfiguration verwalten (Single Point of Truth)
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Apple Notes einsehen, auditieren und durchsuchen
    Notes {
        #[command(subcommand)]
        action: NotesAction,
    },
    /// Apple Mail einsehen, durchsuchen und Signaturen auslesen
    Mail {
        #[command(subcommand)]
        action: MailAction,
    },
    /// Apple Calendar Termine lesen
    Calendar {
        #[command(subcommand)]
        action: CalendarAction,
    },
    /// Apple Reminders offene Aufgaben lesen
    Reminders {
        #[command(subcommand)]
        action: RemindersAction,
    },
    /// Apple Shortcuts (Kurzbefehle) einsehen (Ausführung nur über allowlist-geprüften Plan)
    Shortcuts {
        #[command(subcommand)]
        action: ShortcutsAction,
    },
    /// Transaktionale Änderungspläne (Plan → Inspect → Approve → Apply)
    Plan {
        #[command(subcommand)]
        action: PlanAction,
    },
    /// Anwendungsübergreifende Context Packs für KI-Briefings
    Context {
        #[command(subcommand)]
        action: ContextAction,
    },
    /// Audit-Journal ausgeführter Änderungspläne anzeigen
    History {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Startet den lokalen MCP-Server über stdio (Read + Plan-Create only)
    Mcp,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Legt eine Standard-config.json an (~/.config/macfly/config.json)
    Init {
        #[arg(long)]
        force: bool,
    },
    /// Zeigt den Pfad der aktiven Konfigurationsdatei
    Path,
    /// Zeigt die aktuell geladene Konfiguration als JSON
    Show,
}

#[derive(Subcommand)]
enum NotesAction {
    /// Kompakte Übersicht aller Accounts & Ordner (inkl. Hierarchiepfad)
    Overview,
    /// Aufräum-Audit: leere Ordner, 1-Notiz-Ordner, Duplikat-Titel, älteste Notizen
    Audit {
        /// Zeigt alle Einträge statt kompakter Vorschau
        #[arg(long)]
        details: bool,
    },
    /// Listet Notizen (Metadaten, sortiert nach letzter Änderung)
    List {
        #[arg(long)]
        folder: Option<String>,
        #[arg(long)]
        account: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Sucht Notizen nach Titel (case-insensitive)
    Search {
        query: String,
        #[arg(long)]
        folder: Option<String>,
        #[arg(long)]
        account: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Liest eine einzelne Notiz inkl. Fidelity-Guard und DLP-Redaction
    Read {
        id: String,
        #[arg(long)]
        raw_html: bool,
        #[arg(long)]
        no_redact: bool,
    },
}

#[derive(Subcommand)]
enum MailAction {
    /// Übersicht über Accounts, Postfächer (Ungelesen) und Apple-Mail-Signaturen
    Overview,
    /// Zeigt die direkt in Apple Mail gepflegten Signaturen/Footer
    Signatures,
    /// Listet E-Mails (Default: INBOX)
    List {
        #[arg(long)]
        account: Option<String>,
        #[arg(long, default_value = "INBOX")]
        mailbox: String,
        #[arg(long)]
        unread: bool,
        #[arg(long, default_value_t = 25)]
        limit: usize,
    },
    /// Sucht E-Mails nach Betreff oder Absender
    Search {
        query: String,
        #[arg(long)]
        account: Option<String>,
        #[arg(long, default_value = "INBOX")]
        mailbox: String,
        #[arg(long, default_value_t = 25)]
        limit: usize,
    },
    /// Liest eine einzelne E-Mail per local_id (DLP-redaktiert & untrusted-markiert)
    Read {
        local_id: String,
        #[arg(long)]
        account: Option<String>,
        #[arg(long)]
        mailbox: Option<String>,
        #[arg(long)]
        no_redact: bool,
    },
}

#[derive(Subcommand)]
enum CalendarAction {
    /// Kommende Termine der nächsten N Tage
    Events {
        #[arg(long, default_value_t = 7)]
        days: u32,
        #[arg(long)]
        calendar: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum RemindersAction {
    /// Offene Erinnerungen auflisten
    List {
        #[arg(long)]
        list: Option<String>,
        #[arg(long)]
        overdue: bool,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum ShortcutsAction {
    /// Verfügbare macOS-Kurzbefehle inkl. Allowlist-Status anzeigen
    List,
}

#[derive(Subcommand)]
enum PlanAction {
    /// Erstellt einen geprüften Änderungsplan aus JSON-Datei oder stdin
    Create {
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(long)]
        stdin: bool,
        #[arg(long, default_value_t = 900)]
        ttl_secs: u64,
    },
    /// Listet gespeicherte Änderungspläne
    List,
    /// Zeigt Details, Diff-Vorschau und Prüfcode eines Plans
    Inspect { plan_id: String },
    /// Gibt einen Plan nach Prüfung frei (erfordert interaktives TTY oder --confirm-code)
    Approve {
        plan_id: String,
        #[arg(long)]
        confirm_code: Option<String>,
    },
    /// Führt einen zuvor freigegebenen Plan aus und protokolliert im Audit-Journal
    Apply { plan_id: String },
}

#[derive(Subcommand)]
enum ContextAction {
    /// Kompaktes Morgen-/Tages-Briefing (ungelesene Mails + letzte Notizen)
    DailyBrief {
        #[arg(long, default_value_t = 10)]
        mail_limit: usize,
        #[arg(long, default_value_t = 10)]
        notes_limit: usize,
    },
}

#[derive(Serialize)]
struct DoctorReport {
    os: &'static str,
    osascript_available: bool,
    config_path: String,
    config_exists: bool,
    active_profile: String,
    notes_access: String,
    notes_accounts_count: Option<usize>,
    notes_total_count: Option<usize>,
    mail_access: String,
    mail_accounts_count: Option<usize>,
    mail_unread_count: Option<usize>,
    mail_signatures_count: Option<usize>,
    hints: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            match cli.format {
                OutputFormat::Pretty => ui::render_error(&err, cli.color.into()),
                OutputFormat::Json | OutputFormat::Agent => {
                    if let Ok(json) = serde_json::to_string_pretty(&err.to_envelope()) {
                        println!("{json}");
                    }
                }
            }
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), MacflyError> {
    match &cli.command {
        Commands::Config { action } => handle_config(cli, action),
        Commands::Doctor => handle_doctor(cli),
        Commands::Notes { action } => handle_notes(cli, action),
        Commands::Mail { action } => handle_mail(cli, action),
        Commands::Calendar { action } => handle_calendar(cli, action),
        Commands::Reminders { action } => handle_reminders(cli, action),
        Commands::Shortcuts { action } => handle_shortcuts(cli, action),
        Commands::Plan { action } => handle_plan(cli, action),
        Commands::Context { action } => handle_context(cli, action),
        Commands::History { limit } => handle_history(cli, *limit),
        Commands::Mcp => {
            let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
            let (profile_name, profile) = cfg.profile(cli.profile.as_deref())?;
            mcp::run_mcp_server(&cfg, profile_name, profile)
        }
    }
}

fn emit_json<T: Serialize>(format: OutputFormat, val: &T) -> Result<(), MacflyError> {
    match format {
        OutputFormat::Agent => println!("{}", serde_json::to_string(val)?),
        OutputFormat::Json | OutputFormat::Pretty => {
            println!("{}", serde_json::to_string_pretty(val)?)
        }
    }
    Ok(())
}

fn handle_config(cli: &Cli, action: &ConfigAction) -> Result<(), MacflyError> {
    match action {
        ConfigAction::Init { force } => {
            let path = Config::write_default(cli.config.as_deref(), *force)?;
            match cli.format {
                OutputFormat::Pretty => println!("Config angelegt: {}", path.display()),
                _ => emit_json(
                    cli.format,
                    &serde_json::json!({ "ok": true, "path": path.display().to_string() }),
                )?,
            }
            Ok(())
        }
        ConfigAction::Path => {
            let (_, path, exists) = Config::load_or_default(cli.config.as_deref())?;
            match cli.format {
                OutputFormat::Pretty => println!(
                    "{} ({})",
                    path.display(),
                    if exists {
                        "vorhanden"
                    } else {
                        "Default aktiv, noch nicht angelegt"
                    }
                ),
                _ => emit_json(
                    cli.format,
                    &serde_json::json!({ "path": path.display().to_string(), "exists": exists }),
                )?,
            }
            Ok(())
        }
        ConfigAction::Show => {
            let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
            emit_json(cli.format, &cfg)
        }
    }
}

fn handle_doctor(cli: &Cli) -> Result<(), MacflyError> {
    let (cfg, cfg_path, cfg_exists) = Config::load_or_default(cli.config.as_deref())?;
    let (profile_name, profile) = cfg.profile(cli.profile.as_deref())?;

    let osascript_ok = Command::new("which")
        .arg("osascript")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let mut hints = Vec::new();
    let notes_adapter = NotesAdapter::new(cfg.script_timeout_secs);
    let (notes_access, notes_acc, notes_total) = match notes_adapter.overview(profile_name, profile)
    {
        Ok(ov) => (
            "ok".to_string(),
            Some(ov.total_accounts),
            Some(ov.total_notes),
        ),
        Err(e) => {
            if let Some(h) = e.hint() {
                hints.push(h);
            }
            (e.code().to_string(), None, None)
        }
    };

    let mail_adapter = MailAdapter::new(cfg.script_timeout_secs);
    let (mail_access, mail_acc, mail_unread, mail_sigs) =
        match mail_adapter.overview(profile_name, profile) {
            Ok(ov) => (
                "ok".to_string(),
                Some(ov.total_accounts),
                Some(ov.total_unread),
                Some(ov.signatures.len()),
            ),
            Err(e) => {
                if let Some(h) = e.hint() {
                    hints.push(h);
                }
                (e.code().to_string(), None, None, None)
            }
        };

    let report = DoctorReport {
        os: std::env::consts::OS,
        osascript_available: osascript_ok,
        config_path: cfg_path.display().to_string(),
        config_exists: cfg_exists,
        active_profile: profile_name.to_string(),
        notes_access,
        notes_accounts_count: notes_acc,
        notes_total_count: notes_total,
        mail_access,
        mail_accounts_count: mail_acc,
        mail_unread_count: mail_unread,
        mail_signatures_count: mail_sigs,
        hints,
    };

    match cli.format {
        OutputFormat::Pretty => ui::render_doctor(
            report.os,
            report.osascript_available,
            &report.config_path,
            report.config_exists,
            &report.active_profile,
            &report.notes_access,
            report.notes_accounts_count,
            report.notes_total_count,
            &report.mail_access,
            report.mail_accounts_count,
            report.mail_unread_count,
            report.mail_signatures_count,
            &report.hints,
            cli.color.into(),
        ),
        _ => emit_json(cli.format, &report)?,
    }
    Ok(())
}

fn handle_notes(cli: &Cli, action: &NotesAction) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let (profile_name, profile) = cfg.profile(cli.profile.as_deref())?;
    let adapter = NotesAdapter::new(cfg.script_timeout_secs);

    match action {
        NotesAction::Overview => {
            let ov = adapter.overview(profile_name, profile)?;
            match cli.format {
                OutputFormat::Pretty => ui::render_notes_overview(&ov, cli.color.into()),
                _ => emit_json(cli.format, &ov)?,
            }
            Ok(())
        }
        NotesAction::Audit { details } => {
            let rep = adapter.audit(profile_name, profile)?;
            match cli.format {
                OutputFormat::Pretty => ui::render_notes_audit(&rep, *details, cli.color.into()),
                _ => emit_json(cli.format, &rep)?,
            }
            Ok(())
        }
        NotesAction::List {
            folder,
            account,
            limit,
        } => {
            let res = adapter.list(
                profile_name,
                profile,
                &ListNotesQuery {
                    account: account.clone(),
                    folder: folder.clone(),
                    query: None,
                    limit: *limit,
                },
            )?;
            print_notes_list(cli.format, &res)
        }
        NotesAction::Search {
            query,
            folder,
            account,
            limit,
        } => {
            let res = adapter.list(
                profile_name,
                profile,
                &ListNotesQuery {
                    account: account.clone(),
                    folder: folder.clone(),
                    query: Some(query.clone()),
                    limit: *limit,
                },
            )?;
            print_notes_list(cli.format, &res)
        }
        NotesAction::Read {
            id,
            raw_html,
            no_redact,
        } => {
            let note = adapter.read(profile_name, profile, id, *raw_html, *no_redact)?;
            match cli.format {
                OutputFormat::Pretty => {
                    println!("Titel:    {}", note.title);
                    println!("Ordner:   {} ({})", note.folder_path, note.account);
                    println!("ID:       {}", note.id);
                    println!("Revision: {}", note.revision);
                    if let Some(m) = &note.modified_at {
                        println!("Geändert: {m}");
                    }
                    if !note.fidelity.blocked_mutations.is_empty() {
                        println!(
                            "Fidelity: Rich-Content erkannt (blockiert: {})",
                            note.fidelity.blocked_mutations.join(", ")
                        );
                    }
                    if !note.redactions.is_empty() {
                        println!("DLP:      {:?}", note.redactions);
                    }
                    println!("------------------------------------------------------------");
                    println!("{}", note.body_text);
                }
                _ => emit_json(cli.format, &note)?,
            }
            Ok(())
        }
    }
}

fn handle_mail(cli: &Cli, action: &MailAction) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let (profile_name, profile) = cfg.profile(cli.profile.as_deref())?;
    let adapter = MailAdapter::new(cfg.script_timeout_secs);

    match action {
        MailAction::Overview => {
            let ov = adapter.overview(profile_name, profile)?;
            match cli.format {
                OutputFormat::Pretty => ui::render_mail_overview(&ov, cli.color.into()),
                _ => emit_json(cli.format, &ov)?,
            }
            Ok(())
        }
        MailAction::Signatures => {
            let ov = adapter.overview(profile_name, profile)?;
            match cli.format {
                OutputFormat::Pretty => {
                    println!("Signaturen aus Apple Mail (Single Point of Truth):");
                    for s in &ov.signatures {
                        println!("\n  [{}] ({} Zeichen)", s.name, s.char_count);
                        println!("  {}", s.content_preview.replace('\n', "\n  "));
                    }
                }
                _ => emit_json(cli.format, &ov.signatures)?,
            }
            Ok(())
        }
        MailAction::List {
            account,
            mailbox,
            unread,
            limit,
        } => {
            let res = adapter.list(
                profile_name,
                profile,
                &ListMailQuery {
                    account: account.clone(),
                    mailbox: Some(mailbox.clone()),
                    unread_only: *unread,
                    query: None,
                    limit: *limit,
                    scan_limit: 150,
                },
            )?;
            print_mail_list(cli.format, &res)
        }
        MailAction::Search {
            query,
            account,
            mailbox,
            limit,
        } => {
            let res = adapter.list(
                profile_name,
                profile,
                &ListMailQuery {
                    account: account.clone(),
                    mailbox: Some(mailbox.clone()),
                    unread_only: false,
                    query: Some(query.clone()),
                    limit: *limit,
                    scan_limit: 250,
                },
            )?;
            print_mail_list(cli.format, &res)
        }
        MailAction::Read {
            local_id,
            account,
            mailbox,
            no_redact,
        } => {
            let msg = adapter.read(
                profile_name,
                profile,
                local_id,
                account.as_deref(),
                mailbox.as_deref(),
                *no_redact,
            )?;
            match cli.format {
                OutputFormat::Pretty => {
                    println!("Betreff:  {}", msg.subject);
                    println!("Von:      {}", msg.sender);
                    if !msg.to.is_empty() {
                        println!("An:       {}", msg.to.join(", "));
                    }
                    println!("Postfach: {}/{}", msg.account, msg.mailbox);
                    println!("Local-ID: {}  |  Stabile ID: {}", msg.local_id, msg.id);
                    if !msg.redactions.is_empty() {
                        println!("DLP:      {:?}", msg.redactions);
                    }
                    println!("Hinweis:  {}", msg.content_warning);
                    println!("------------------------------------------------------------");
                    println!("{}", msg.body_text);
                }
                _ => emit_json(cli.format, &msg)?,
            }
            Ok(())
        }
    }
}

fn handle_plan(cli: &Cli, action: &PlanAction) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let (profile_name, profile) = cfg.profile(cli.profile.as_deref())?;
    let store = PlanStore::default_store();

    match action {
        PlanAction::Create {
            input,
            stdin,
            ttl_secs,
        } => {
            let raw = if *stdin {
                let mut buf = String::new();
                io::stdin().read_to_string(&mut buf)?;
                buf
            } else if let Some(path) = input {
                fs::read_to_string(path)?
            } else {
                return Err(MacflyError::PlanError(
                    "Bitte --input <datei.json> oder --stdin angeben".to_string(),
                ));
            };
            let proposal: PlanProposal = serde_json::from_str(&raw)?;
            let plan = store.create(profile_name, profile, proposal, *ttl_secs)?;
            match cli.format {
                OutputFormat::Pretty => ui::render_plan(&plan, cli.color.into()),
                _ => emit_json(cli.format, &plan)?,
            }
            Ok(())
        }
        PlanAction::List => {
            let plans = store.list()?;
            match cli.format {
                OutputFormat::Pretty => {
                    if plans.is_empty() {
                        println!("Keine Änderungspläne vorhanden.");
                    } else {
                        for p in &plans {
                            println!(
                                "  {}  [{:?}]  {:?}  ({} Ops)  {}",
                                p.plan_id,
                                p.status,
                                p.risk_level,
                                p.operations.len(),
                                p.summary
                            );
                        }
                    }
                }
                _ => emit_json(cli.format, &plans)?,
            }
            Ok(())
        }
        PlanAction::Inspect { plan_id } => {
            let plan = store.load(plan_id)?;
            match cli.format {
                OutputFormat::Pretty => ui::render_plan(&plan, cli.color.into()),
                _ => emit_json(cli.format, &plan)?,
            }
            Ok(())
        }
        PlanAction::Approve {
            plan_id,
            confirm_code,
        } => {
            let plan = store.load(plan_id)?;
            let code = match confirm_code {
                Some(c) => c.clone(),
                None => {
                    if !io::stdin().is_terminal() {
                        return Err(MacflyError::ApprovalError(
                            "Freigabe ohne --confirm-code erfordert ein interaktives Terminal (Schutz gegen Self-Approval)."
                                .to_string(),
                        ));
                    }
                    println!("Änderungsplan prüfen: {}", plan.plan_id);
                    println!("Risiko: {:?} | Reversibilität: {:?}", plan.risk_level, plan.reversibility);
                    for line in &plan.preview_lines {
                        println!("  • {line}");
                    }
                    print!(
                        "\nZur Freigabe Prüfcode '{}' eingeben: ",
                        plan.approval_code
                    );
                    io::stdout().flush()?;
                    let mut answer = String::new();
                    io::stdin().read_line(&mut answer)?;
                    answer.trim().to_string()
                }
            };
            let approved = store.approve(plan_id, &code)?;
            match cli.format {
                OutputFormat::Pretty => {
                    println!(
                        "Plan '{}' freigegeben. Ausführen mit:\n  macfly plan apply {}",
                        approved.plan_id, approved.plan_id
                    );
                }
                _ => emit_json(cli.format, &approved)?,
            }
            Ok(())
        }
        PlanAction::Apply { plan_id } => {
            let journal = AuditJournal::default_journal();
            let report = apply_approved_plan(
                &store,
                &journal,
                plan_id,
                profile_name,
                profile,
                cfg.script_timeout_secs,
            )?;
            match cli.format {
                OutputFormat::Pretty => ui::render_apply_report(&report, cli.color.into()),
                _ => emit_json(cli.format, &report)?,
            }
            Ok(())
        }
    }
}

fn handle_calendar(cli: &Cli, action: &CalendarAction) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let adapter = CalendarAdapter::new(cfg.script_timeout_secs);
    match action {
        CalendarAction::Events {
            days,
            calendar,
            limit,
        } => {
            let res = adapter.events(*days, calendar.as_deref(), *limit)?;
            match cli.format {
                OutputFormat::Pretty => {
                    println!(
                        "Termine (nächste {} Tage: {} angezeigt von {})",
                        res.days, res.returned, res.total_matched
                    );
                    for ev in &res.events {
                        let start = ev
                            .start_at
                            .as_deref()
                            .and_then(|s| s.get(..16))
                            .unwrap_or("?");
                        println!("  • {start}  [{}]  {}", ev.calendar, ev.title);
                    }
                }
                _ => emit_json(cli.format, &res)?,
            }
            Ok(())
        }
    }
}

fn handle_reminders(cli: &Cli, action: &RemindersAction) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let adapter = RemindersAdapter::new(cfg.script_timeout_secs);
    match action {
        RemindersAction::List {
            list,
            overdue,
            limit,
        } => {
            let res = adapter.list(list.as_deref(), *overdue, *limit)?;
            match cli.format {
                OutputFormat::Pretty => {
                    println!(
                        "Offene Erinnerungen ({} angezeigt von {})",
                        res.returned, res.total_matched
                    );
                    for r in &res.reminders {
                        let flag = if r.overdue { "!" } else { " " };
                        let due = r
                            .due_at
                            .as_deref()
                            .and_then(|s| s.get(..10))
                            .unwrap_or("----------");
                        println!("  {flag} {due}  [{}]  {}", r.list, r.title);
                    }
                }
                _ => emit_json(cli.format, &res)?,
            }
            Ok(())
        }
    }
}

fn handle_shortcuts(cli: &Cli, action: &ShortcutsAction) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let (profile_name, profile) = cfg.profile(cli.profile.as_deref())?;
    let adapter = ShortcutsAdapter::new(cfg.script_timeout_secs);
    match action {
        ShortcutsAction::List => {
            let res = adapter.list(profile_name, profile)?;
            match cli.format {
                OutputFormat::Pretty => ui::render_shortcuts_list(&res, cli.color.into()),
                _ => emit_json(cli.format, &res)?,
            }
            Ok(())
        }
    }
}

fn handle_context(cli: &Cli, action: &ContextAction) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let (profile_name, profile) = cfg.profile(cli.profile.as_deref())?;
    let notes = NotesAdapter::new(cfg.script_timeout_secs);
    let mail = MailAdapter::new(cfg.script_timeout_secs);
    let cal = CalendarAdapter::new(cfg.script_timeout_secs);
    let rem = RemindersAdapter::new(cfg.script_timeout_secs);

    match action {
        ContextAction::DailyBrief {
            mail_limit,
            notes_limit,
        } => {
            let mut warnings = Vec::new();
            let upcoming_events = match cal.events(2, None, 15) {
                Ok(c) => c.events,
                Err(e) => {
                    warnings.push(format!("Calendar nicht verfügbar: {e}"));
                    Vec::new()
                }
            };
            let open_reminders = match rem.list(None, false, 15) {
                Ok(r) => r.reminders,
                Err(e) => {
                    warnings.push(format!("Reminders nicht verfügbar: {e}"));
                    Vec::new()
                }
            };
            let unread_mails = match mail.list(
                profile_name,
                profile,
                &ListMailQuery {
                    mailbox: Some("INBOX".to_string()),
                    unread_only: true,
                    limit: *mail_limit,
                    scan_limit: 150,
                    ..Default::default()
                },
            ) {
                Ok(m) => m.messages,
                Err(e) => {
                    warnings.push(format!("Mail nicht verfügbar: {e}"));
                    Vec::new()
                }
            };

            let recent_notes = match notes.list(
                profile_name,
                profile,
                &ListNotesQuery {
                    limit: *notes_limit,
                    ..Default::default()
                },
            ) {
                Ok(n) => n.notes,
                Err(e) => {
                    warnings.push(format!("Notes nicht verfügbar: {e}"));
                    Vec::new()
                }
            };

            let brief = DailyBrief {
                schema_version: "1.0".to_string(),
                profile: profile_name.to_string(),
                generated_at_unix: now_unix(),
                upcoming_events,
                open_reminders,
                unread_mail_count: unread_mails.len(),
                unread_mails,
                recent_notes,
                warnings,
            };

            match cli.format {
                OutputFormat::Pretty => ui::render_daily_brief(&brief, cli.color.into()),
                _ => emit_json(cli.format, &brief)?,
            }
            Ok(())
        }
    }
}

fn handle_history(cli: &Cli, limit: usize) -> Result<(), MacflyError> {
    let journal = AuditJournal::default_journal();
    let entries = journal.recent(limit)?;
    match cli.format {
        OutputFormat::Pretty => {
            if entries.is_empty() {
                println!("Audit-Journal ist noch leer.");
            } else {
                for e in &entries {
                    println!(
                        "  {}  [{}]  {} ({} Ops)",
                        e.timestamp_unix, e.status, e.plan_id, e.operations_count
                    );
                    for d in &e.details {
                        println!("      - {d}");
                    }
                }
            }
        }
        _ => emit_json(cli.format, &entries)?,
    }
    Ok(())
}

fn print_notes_list(
    format: OutputFormat,
    res: &macfly_adapters::notes::NotesListResult,
) -> Result<(), MacflyError> {
    match format {
        OutputFormat::Pretty => {
            println!(
                "Notizen: {} angezeigt (von {} Treffern, Profil: {})",
                res.returned, res.total_matched, res.profile
            );
            println!();
            for n in &res.notes {
                let date = n
                    .modified_at
                    .as_deref()
                    .and_then(|s| s.get(..10))
                    .unwrap_or("----------");
                let folder_disp = if n.folder_path.is_empty() {
                    &n.folder
                } else {
                    &n.folder_path
                };
                println!("  {date}  [{}/{}]  {}", n.account, folder_disp, n.title);
                println!("              id: {}", n.id);
            }
        }
        _ => emit_json(format, res)?,
    }
    Ok(())
}

fn print_mail_list(
    format: OutputFormat,
    res: &macfly_adapters::mail::MailListResult,
) -> Result<(), MacflyError> {
    match format {
        OutputFormat::Pretty => {
            println!(
                "E-Mails: {} angezeigt (von {} Treffern, Profil: {})",
                res.returned, res.total_matched, res.profile
            );
            println!();
            for m in &res.messages {
                let date = m
                    .date_received
                    .as_deref()
                    .and_then(|s| s.get(..10))
                    .unwrap_or("----------");
                let marker = if m.read { " " } else { "*" };
                println!(
                    "  {marker} {date}  [{}/{}]  {}",
                    m.account, m.mailbox, m.subject
                );
                println!("              von: {}  |  local_id: {}", m.sender, m.local_id);
            }
        }
        _ => emit_json(format, res)?,
    }
    Ok(())
}
