mod mcp;
mod ui;

use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use clap::{Parser, Subcommand, ValueEnum};
use macfly_adapters::calendar::{CalendarAdapter, RemindersAdapter};
use macfly_adapters::contacts::ContactsAdapter;
use macfly_adapters::context::{
    build_daily_brief, build_meeting_prep, build_person_context, build_weekly_review,
};
use macfly_adapters::executor::apply_approved_plan;
use macfly_adapters::mail::{ListMailQuery, MailAdapter};
use macfly_adapters::music::MusicAdapter;
use macfly_adapters::notes::{ListNotesQuery, NotesAdapter};
use macfly_adapters::photos::PhotosAdapter;
use macfly_adapters::safari::SafariAdapter;
use macfly_adapters::shortcuts::ShortcutsAdapter;
use macfly_core::config::{Config, Preset, data_dir};
use macfly_core::error::MacflyError;
use macfly_core::import::{
    is_supported_note_file, is_supported_reminder_file, parse_notes_from_str,
    parse_reminders_from_str,
};
use macfly_core::journal::AuditJournal;
use macfly_core::plan::{
    ACTION_RISKS, ImageFormat, PhotoConvert, PlanOperation, PlanProposal, PlanStore, SidecarFormat,
    StoredPlan, preset_allows,
};
use macfly_core::plan_text::Lang;
use macfly_core::report_export::{
    daily_brief_to_typst, person_context_to_markdown, person_context_to_typst,
    weekly_review_to_typst,
};
use macfly_core::schedule::{
    job_label, launch_agents_dir, list_installed_jobs, render_launchd_plist,
};
use macfly_core::skill::{export_skill_file, render_agent_skill_markdown};
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
    /// Apple Music: Playlists und Titel lesen, Playlists per Plan anlegen/befüllen
    Music {
        #[command(subcommand)]
        action: MusicAction,
    },
    /// Apple Shortcuts (Kurzbefehle) einsehen (Ausführung nur über allowlist-geprüften Plan)
    Shortcuts {
        #[command(subcommand)]
        action: ShortcutsAction,
    },
    /// Apple Contacts (Kontakte) durchsuchen, auditieren und exportieren
    Contacts {
        #[command(subcommand)]
        action: ContactsAction,
    },
    /// Apple Safari offene Tabs auslesen, als Markdown exportieren oder nach Apple Notes migrieren
    Safari {
        #[command(subcommand)]
        action: SafariAction,
    },
    /// Apple Photos: Alben und Fotos auflisten, Export (Originale, Metadaten, Umwandlung) per Plan
    Photos {
        #[command(subcommand)]
        action: PhotosAction,
    },
    /// Transaktionale Änderungspläne (Plan → Inspect → Approve → Apply)
    Plan {
        #[command(subcommand)]
        action: PlanAction,
    },
    /// Anwendungsübergreifende Context Packs (Daily Brief, 360° Person, Meeting Prep, Weekly Review)
    Context {
        #[command(subcommand)]
        action: ContextAction,
    },
    /// Druckfertige Dossiers & Reports (Typst / renderreport / PDF / Markdown)
    Report {
        #[command(subcommand)]
        action: ReportAction,
    },
    /// Agent-Skill (SKILL.md) für Claude Code, Codex, Cursor oder lokale Agenten ausgeben
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
    /// Zeitgesteuerte lokale Routinen über macOS launchd (LaunchAgents) verwalten
    Schedule {
        #[command(subcommand)]
        action: ScheduleAction,
    },
    /// Zeigt in Klartext, was das aktive Profil darf (Sicherheitsstufe, Freigaben, Sperren, DLP)
    Policy,
    /// Audit-Journal ausgeführter Änderungspläne anzeigen
    History {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Startet den lokalen MCP-Server über stdio (Read, Plan-Create/Inspect, Apply freigegebener Pläne)
    Mcp,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Legt eine Standard-config.json an (~/.config/macfly/config.json)
    Init {
        #[arg(long)]
        force: bool,
        /// Sicherheitsstufe: strict, balanced (Standard) oder open. Ohne Angabe wird im Terminal gefragt.
        #[arg(long, value_parser = ["strict", "balanced", "open"])]
        preset: Option<String>,
        /// Sprache der Ausgaben: en (Standard) oder de. Ohne Angabe wird im Terminal gefragt.
        #[arg(long, value_parser = ["en", "de"])]
        language: Option<String>,
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
    /// Exportiert Notizen als Markdown-Dateien (inkl. YAML-Frontmatter) in ein Verzeichnis
    Export {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        folder: Option<String>,
        #[arg(long)]
        account: Option<String>,
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long)]
        no_redact: bool,
    },
    /// Importiert Dateien (.md, .txt, .html, .json, .jsonl, .csv) oder ein Verzeichnis als geprüften Änderungsplan nach Apple Notes
    Import {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        folder: String,
        #[arg(long)]
        account: Option<String>,
        /// Unterordner rekursiv durchsuchen
        #[arg(long)]
        recursive: bool,
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
    /// Exportiert E-Mails als Markdown-Dateien (inkl. YAML-Frontmatter) in ein Verzeichnis
    Export {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        account: Option<String>,
        #[arg(long, default_value = "INBOX")]
        mailbox: String,
        #[arg(long)]
        query: Option<String>,
        #[arg(long)]
        unread: bool,
        #[arg(long, default_value_t = 25)]
        limit: usize,
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
    /// Exportiert kommende Termine als Markdown-Übersicht (inkl. YAML-Frontmatter)
    Export {
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 14)]
        days: u32,
        #[arg(long)]
        calendar: Option<String>,
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
    /// Exportiert offene Erinnerungen als Markdown-Checkliste (`- [ ] ...`)
    Export {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        list: Option<String>,
        #[arg(long)]
        overdue: bool,
    },
    /// Importiert Aufgaben (.md, .txt, .json, .jsonl, .csv) als geprüften Änderungsplan nach Apple Reminders
    Import {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        list: Option<String>,
    },
}

#[derive(Subcommand)]
enum MusicAction {
    /// Playlists der Mediathek auflisten
    Playlists,
    /// Titel der Mediathek oder einer Playlist auflisten (inkl. Track-ID)
    Tracks {
        #[arg(long)]
        playlist: Option<String>,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    /// Hörstatistik aus den Zählern der Mediathek (Top-Listen, Übersprungenes, lange nicht gehört, nie gespielt)
    Stats {
        /// Länge der Top-Listen
        #[arg(long, default_value_t = 20)]
        top: usize,
        /// Ab wie vielen Tagen ohne Wiedergabe ein Titel als „lange nicht gehört“ gilt
        #[arg(long, default_value_t = 365)]
        stale_days: u64,
        /// Alle Titel mit Zählern zusätzlich als CSV schreiben (z. B. für Numbers/Excel)
        #[arg(long)]
        csv: Option<PathBuf>,
    },
    /// Erstellt einen Plan (`music_playlist_add`): Playlist anlegen (falls nötig) und Titel per ID hinzufügen
    Add {
        #[arg(long)]
        playlist: String,
        /// Track-IDs aus `macfly music tracks`
        #[arg(required = true)]
        track_ids: Vec<String>,
    },
}

#[derive(Subcommand)]
enum ShortcutsAction {
    /// Verfügbare macOS-Kurzbefehle inkl. Allowlist-Status anzeigen
    List,
}

#[derive(Subcommand)]
enum ContactsAction {
    /// Kontakte auflisten
    List {
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Kontakte nach Name, Firma, E-Mail oder Telefon suchen
    Search {
        query: String,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Aufräum-Audit: doppelte Namen, doppelte E-Mails, Kontakte ohne E-Mail/Telefon
    Audit {
        #[arg(long)]
        details: bool,
    },
    /// Exportiert Kontakte als Markdown-Steckbriefe (mit YAML-Frontmatter) in ein Verzeichnis
    Export {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        query: Option<String>,
        #[arg(long, default_value_t = 0)]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum SafariAction {
    /// Offene Safari-Tabs (Titel & URL) auflisten
    Tabs {
        #[arg(long)]
        query: Option<String>,
    },
    /// Offene Safari-Tabs als Markdown-Linkliste exportieren
    Export {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        query: Option<String>,
    },
    /// Migriert offene Safari-Tabs über einen geprüften Änderungsplan als Sammelnotiz nach Apple Notes
    ToNote {
        #[arg(long, default_value = "00 · Eingang")]
        folder: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        query: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SidecarArg {
    None,
    Xmp,
    Json,
    Both,
}

impl From<SidecarArg> for SidecarFormat {
    fn from(v: SidecarArg) -> Self {
        match v {
            SidecarArg::None => Self::None,
            SidecarArg::Xmp => Self::Xmp,
            SidecarArg::Json => Self::Json,
            SidecarArg::Both => Self::Both,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ConvertArg {
    Jpeg,
    Png,
    Tiff,
    Heic,
}

impl From<ConvertArg> for ImageFormat {
    fn from(v: ConvertArg) -> Self {
        match v {
            ConvertArg::Jpeg => Self::Jpeg,
            ConvertArg::Png => Self::Png,
            ConvertArg::Tiff => Self::Tiff,
            ConvertArg::Heic => Self::Heic,
        }
    }
}

#[derive(Subcommand)]
enum PhotosAction {
    /// Alben mit Anzahl der Fotos auflisten
    Albums,
    /// Fotos und Videos mit Metadaten auflisten (neueste zuerst)
    List {
        #[arg(long)]
        album: Option<String>,
        /// Schlagwort (Teilwort, ohne Groß-/Kleinschreibung)
        #[arg(long)]
        keyword: Option<String>,
        /// Nur Aufnahmen ab diesem Datum (JJJJ-MM-TT)
        #[arg(long)]
        since: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Exportplan erstellen: Originale oder gerenderte Fotos, optional mit XMP/JSON-Metadaten und Umwandlung
    Export {
        /// Zielordner (neue Dateien, nichts wird überschrieben)
        #[arg(long)]
        to: String,
        #[arg(long)]
        album: Option<String>,
        #[arg(long)]
        keyword: Option<String>,
        #[arg(long)]
        since: Option<String>,
        /// Einzelne Medien-IDs (aus `photos list`), mehrfach angebbar
        #[arg(long = "id")]
        ids: Vec<String>,
        /// Höchstens so viele Fotos exportieren (0 = alle Treffer)
        #[arg(long, default_value_t = 0)]
        limit: usize,
        /// Originaldateien statt gerenderter JPEGs mit Bearbeitungen
        #[arg(long)]
        originals: bool,
        /// Begleitdatei mit Titel, Beschreibung, Schlagwörtern, Datum und Ort
        #[arg(long, value_enum, default_value = "none")]
        sidecar: SidecarArg,
        /// Bilder mit dem macOS-Bordmittel `sips` umwandeln
        #[arg(long, value_enum)]
        convert: Option<ConvertArg>,
        /// Längste Kante in Pixeln (nur verkleinern, braucht --convert)
        #[arg(long)]
        max_size: Option<u32>,
        /// Qualität 1–100 für JPEG und HEIC (braucht --convert)
        #[arg(long)]
        quality: Option<u8>,
    },
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
    /// Zeigt Details, Diff-Vorschau und Freigabestatus eines Plans
    Inspect { plan_id: String },
    /// Gibt einen Plan nach Prüfung frei und führt ihn sofort aus (erfordert immer ein interaktives TTY)
    Approve {
        plan_id: String,
        /// Nur freigeben, nicht ausführen (Ausführung später mit `macfly plan apply`)
        #[arg(long)]
        no_apply: bool,
        /// Alle Einzelheiten und technischen Angaben anzeigen
        #[arg(long)]
        details: bool,
    },
    /// Führt einen zuvor freigegebenen Plan aus und protokolliert im Audit-Journal
    Apply {
        plan_id: String,
        /// Setzt einen teilweise ausgeführten Plan (partially_applied) ab der ersten offenen Operation fort
        #[arg(long)]
        resume: bool,
        /// Technische Angaben (Prüfsummen, Dateipfade) mit anzeigen
        #[arg(long)]
        details: bool,
    },
}

#[derive(Subcommand)]
enum ContextAction {
    /// Kompaktes Morgen-/Tages-Briefing (Termine 48h + Erinnerungen + ungelesene Mails + Notizen)
    DailyBrief {
        #[arg(long, default_value_t = 10)]
        mail_limit: usize,
        #[arg(long, default_value_t = 10)]
        notes_limit: usize,
    },
    /// 360°-Kunden-/Personen-Dossier über Contacts, Mail, Notes, Calendar und Reminders
    Person {
        query: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Bereitet den nächsten (oder gesuchten) Kalendertermin mit Mails, Kontakten & Notizen vor
    MeetingPrep {
        #[arg(long)]
        query: Option<String>,
        /// Erzeugt direkt einen Änderungsplan für eine vorbereitete Meeting-Notiz
        #[arg(long)]
        create_note: bool,
        #[arg(long, default_value = "00 · Eingang")]
        folder: String,
    },
    /// Wochenrückblick über geänderte Notizen, ungelesene Mails, überfällige Aufgaben & Safari-Tabs
    WeeklyReview,
}

#[derive(Subcommand)]
enum ReportAction {
    /// Erzeugt ein 360°-Dossier für eine Person/Firma als Typst (.typ), Markdown (.md) oder PDF (.pdf)
    Person {
        query: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Erzeugt das Daily Brief als Typst (.typ) oder PDF (.pdf)
    DailyBrief {
        #[arg(long)]
        output: PathBuf,
    },
    /// Erzeugt das Weekly Review als Typst (.typ) oder PDF (.pdf)
    WeeklyReview {
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum SkillAction {
    /// Gibt die fertige SKILL.md auf stdout aus
    Show,
    /// Speichert SKILL.md in ein Zielverzeichnis oder eine Datei (z. B. .claude/skills/macfly)
    Export {
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ScheduleTask {
    DailyBrief,
    WeeklyReview,
    NotesAudit,
}

#[derive(Subcommand)]
enum ScheduleAction {
    /// Listet installierte MacFly LaunchAgents (~/Library/LaunchAgents/com.casoon.macfly.*.plist)
    List,
    /// Richtet eine tägliche lokale Routine über macOS launchd ein
    Install {
        #[arg(long)]
        name: String,
        #[arg(long, value_enum)]
        task: ScheduleTask,
        #[arg(long, default_value_t = 7)]
        hour: u8,
        #[arg(long, default_value_t = 30)]
        minute: u8,
        /// Nur die generierte .plist anzeigen, ohne sie zu installieren
        #[arg(long)]
        dry_run: bool,
    },
    /// Entfernt einen MacFly LaunchAgent
    Remove {
        #[arg(long)]
        name: String,
    },
}

#[derive(Serialize)]
struct DoctorReport {
    os: &'static str,
    osascript_available: bool,
    config_path: String,
    config_exists: bool,
    active_profile: String,
    preset: &'static str,
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
    if let Ok((cfg, _, _)) = Config::load_or_default(cli.config.as_deref()) {
        Lang::set_configured(cfg.language);
    }
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
        Commands::Music { action } => handle_music(cli, action),
        Commands::Shortcuts { action } => handle_shortcuts(cli, action),
        Commands::Contacts { action } => handle_contacts(cli, action),
        Commands::Safari { action } => handle_safari(cli, action),
        Commands::Photos { action } => handle_photos(cli, action),
        Commands::Plan { action } => handle_plan(cli, action),
        Commands::Context { action } => handle_context(cli, action),
        Commands::Report { action } => handle_report(cli, action),
        Commands::Skill { action } => handle_skill(cli, action),
        Commands::Schedule { action } => handle_schedule(cli, action),
        Commands::Policy => handle_policy(cli),
        Commands::History { limit } => handle_history(cli, *limit),
        Commands::Mcp => {
            let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
            let (profile_name, profile) = cfg.profile(cli.profile.as_deref())?;
            mcp::run_mcp_server(&cfg, profile_name, profile)
        }
    }
}

/// Gibt einen Plan aus — nie mit Prüfcode, damit ein Agent ihn nicht weiterreichen kann.
fn emit_plan(cli: &Cli, plan: &StoredPlan) -> Result<(), MacflyError> {
    match cli.format {
        OutputFormat::Pretty => ui::render_plan(plan, cli.color.into()),
        _ => emit_json(cli.format, &plan.for_output())?,
    }
    Ok(())
}

/// `--no-redact` nur, wenn ein Mensch am Terminal sitzt — nicht in Pipes, Skripten oder Agenten-Aufrufen.
fn ensure_redaction_bypass_allowed(no_redact: bool) -> Result<(), MacflyError> {
    if no_redact && !io::stdin().is_terminal() {
        return Err(MacflyError::ScopeDenied(
            "`--no-redact` ist nur in einem interaktiven Terminal erlaubt (Schutz vor ungefilterter Weitergabe an KI-Modelle)."
                .to_string(),
        ));
    }
    Ok(())
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
        ConfigAction::Init {
            force,
            preset,
            language,
        } => {
            let interactive =
                matches!(cli.format, OutputFormat::Pretty) && io::stdin().is_terminal();
            let language = match language {
                Some(name) => Lang::parse(name).unwrap_or_default(),
                None if interactive => ask_language(cli.color.into())?,
                None => Lang::default(),
            };
            Lang::set_configured(language);
            let preset = match preset {
                Some(name) => Preset::parse(name).unwrap_or_default(),
                None if matches!(cli.format, OutputFormat::Pretty) && io::stdin().is_terminal() => {
                    ask_preset(language, cli.color.into())?
                }
                None => Preset::default(),
            };
            let path = Config::write_default(cli.config.as_deref(), *force, preset, language)?;
            match cli.format {
                OutputFormat::Pretty => println!(
                    "Config angelegt: {} (Sicherheitsstufe: {})\nDetails: macfly policy",
                    path.display(),
                    preset.name()
                ),
                _ => emit_json(
                    cli.format,
                    &serde_json::json!({
                        "ok": true,
                        "path": path.display().to_string(),
                        "preset": preset.name()
                    }),
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

fn cancelled_init() -> MacflyError {
    MacflyError::ConfigError {
        path: String::new(),
        detail: "Abgebrochen / cancelled".to_string(),
    }
}

fn ask_language(color: ColorMode) -> Result<Lang, MacflyError> {
    let picked = ui::choose(
        "MacFly · Language / Sprache",
        "config init",
        &[
            ("en", "English", "default"),
            ("de", "Deutsch", "Ausgaben von MacFly auf Deutsch"),
        ],
        color,
    )?;
    Ok(Lang::parse(&picked.ok_or_else(cancelled_init)?).unwrap_or_default())
}

fn ask_preset(lang: Lang, color: ColorMode) -> Result<Preset, MacflyError> {
    let desc = |p: Preset| match (p, lang) {
        (Preset::Strict, Lang::En) => {
            "Every change needs your approval in the terminal. For sensitive mailboxes and work machines."
        }
        (Preset::Balanced, Lang::En) => {
            "Creating notes, folders, mail drafts and reminders runs automatically (up to 5 operations). Moving, appending, deleting and shortcuts need your approval."
        }
        (Preset::Open, Lang::En) => {
            "Also moves, appends and deletes empty folders automatically. For clean-up and migration sessions. Shortcuts still need your approval."
        }
        (p, Lang::De) => p.description(),
    };
    let (heading, note) = match lang {
        Lang::En => (
            "Security level",
            "change later via \"preset\" in the config",
        ),
        Lang::De => (
            "Sicherheitsstufe",
            "später änderbar über \"preset\" in der Config",
        ),
    };
    let default = lang.t(" (Standard)", " (default)");
    let label = |p: Preset| {
        if p == Preset::default() {
            format!("{}{}", p.name(), default)
        } else {
            p.name().to_string()
        }
    };
    // Cursor starts on the first entry: the default goes first.
    let mut order = vec![Preset::default()];
    order.extend(Preset::ALL.into_iter().filter(|p| *p != Preset::default()));
    let labels: Vec<String> = order.iter().map(|p| label(*p)).collect();
    let descs: Vec<&str> = order.iter().map(|p| desc(*p)).collect();
    let choices: Vec<ui::Choice<'_>> = order
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name(), labels[i].as_str(), descs[i]))
        .collect();
    let picked = ui::choose(heading, note, &choices, color)?;
    Ok(Preset::parse(&picked.ok_or_else(cancelled_init)?).unwrap_or_default())
}

#[derive(Serialize)]
struct PolicyReport {
    profile: String,
    preset: &'static str,
    preset_description: &'static str,
    auto_approved_actions: Vec<&'static str>,
    approval_required_actions: Vec<&'static str>,
    max_auto_operations: Option<usize>,
    notes_deny_folders: Vec<String>,
    notes_allow_folders: Vec<String>,
    mail_allow_accounts: Vec<String>,
    mail_deny_mailboxes: Vec<String>,
    shortcuts_allow: Vec<String>,
    dlp_enabled: bool,
    dlp_rules: Vec<String>,
    unredacted_reads: &'static str,
    never: Vec<&'static str>,
}

fn handle_policy(cli: &Cli) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let (profile_name, profile) = cfg.profile(cli.profile.as_deref())?;
    let preset = profile.preset;
    let (auto, manual): (Vec<_>, Vec<_>) = ACTION_RISKS
        .iter()
        .partition(|(_, risk)| preset_allows(preset, *risk));
    let report = PolicyReport {
        profile: profile_name.to_string(),
        preset: preset.name(),
        preset_description: preset.description(),
        auto_approved_actions: auto.iter().map(|(a, _)| *a).collect(),
        approval_required_actions: manual.iter().map(|(a, _)| *a).collect(),
        max_auto_operations: (preset == Preset::Balanced).then_some(5),
        notes_deny_folders: profile.notes.deny_folders.clone(),
        notes_allow_folders: profile.notes.allow_folders.clone(),
        mail_allow_accounts: profile.mail.allow_accounts.clone(),
        mail_deny_mailboxes: profile.mail.deny_mailboxes.clone(),
        shortcuts_allow: profile.shortcuts.allow.clone(),
        dlp_enabled: profile.dlp.enabled,
        dlp_rules: profile.dlp.redact.clone(),
        unredacted_reads: "nur im interaktiven Terminal (--no-redact)",
        never: vec![
            "Mails senden (nur Entwürfe)",
            "Pläne über MCP freigeben",
            "gesperrte Ordner/Postfächer lesen oder ändern",
        ],
    };
    match cli.format {
        OutputFormat::Pretty => ui::render_policy(&report, cli.color.into()),
        _ => emit_json(cli.format, &report)?,
    }
    Ok(())
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
    if profile.preset == Preset::Open {
        hints.push(
            "Sicherheitsstufe 'open': Verschieben, Anhängen und Löschen leerer Ordner laufen ohne Rückfrage. Für den Alltag 'balanced' empfohlen (macfly policy)."
                .to_string(),
        );
    }
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
        preset: profile.preset.name(),
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
            report.preset,
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
            ensure_redaction_bypass_allowed(*no_redact)?;
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
        NotesAction::Export {
            output,
            folder,
            account,
            limit,
            no_redact,
        } => {
            ensure_redaction_bypass_allowed(*no_redact)?;
            let list = adapter.list(
                profile_name,
                profile,
                &ListNotesQuery {
                    account: account.clone(),
                    folder: folder.clone(),
                    query: None,
                    limit: *limit,
                },
            )?;
            fs::create_dir_all(output)?;
            let mut exported_files = Vec::new();
            let mut used_paths = std::collections::HashSet::new();
            for summary in &list.notes {
                let detail = adapter.read(profile_name, profile, &summary.id, false, *no_redact)?;
                let rel_folder = if detail.folder_path.is_empty() {
                    sanitize_filename(&detail.folder)
                } else {
                    detail
                        .folder_path
                        .split('/')
                        .map(sanitize_filename)
                        .collect::<Vec<_>>()
                        .join("/")
                };
                let target_dir = output.join(&rel_folder);
                fs::create_dir_all(&target_dir)?;
                let fname = format!("{}.md", sanitize_filename(&detail.title));
                let file_path = target_dir.join(fname);
                let file_path = unique_export_path(file_path, &mut used_paths);
                let md = format!(
                    "---\ntitle: {:?}\nid: {:?}\naccount: {:?}\nfolder: {:?}\nmodified_at: {:?}\nrevision: {:?}\n---\n\n{}\n",
                    detail.title,
                    detail.id,
                    detail.account,
                    detail.folder_path,
                    detail.modified_at.clone().unwrap_or_default(),
                    detail.revision,
                    detail.body_text
                );
                fs::write(&file_path, md)?;
                exported_files.push(file_path.display().to_string());
            }
            match cli.format {
                OutputFormat::Pretty => {
                    println!(
                        "{} Notiz(en) als Markdown exportiert nach: {}",
                        exported_files.len(),
                        output.display()
                    );
                }
                _ => emit_json(
                    cli.format,
                    &serde_json::json!({
                        "ok": true,
                        "output_dir": output.display().to_string(),
                        "exported_count": exported_files.len(),
                        "files": exported_files,
                    }),
                )?,
            }
            Ok(())
        }
        NotesAction::Import {
            input,
            folder,
            account,
            recursive,
        } => {
            let files = collect_import_files(input, *recursive, is_supported_note_file)?;
            if files.is_empty() {
                return Err(MacflyError::PlanError(format!(
                    "Keine unterstützten Dateien (.md, .txt, .html, .json, .jsonl, .csv) gefunden unter {}",
                    input.display()
                )));
            }
            let mut operations = Vec::new();
            for path in &files {
                let raw = fs::read_to_string(path)?;
                let notes = parse_notes_from_str(path, &raw)?;
                for note in notes {
                    operations.push(PlanOperation::NotesCreate {
                        account: account.clone(),
                        folder: note.folder_override.unwrap_or_else(|| folder.clone()),
                        title: note.title,
                        body: note.body,
                    });
                }
            }
            let store = PlanStore::default_store();
            let proposal = PlanProposal {
                schema_version: "1.0".to_string(),
                summary: Some(format!(
                    "Notes-Import: {} Notiz(en) aus {} Datei(en) nach '{}'",
                    operations.len(),
                    files.len(),
                    folder
                )),
                operations,
            };
            let plan = store.create(profile_name, profile, proposal, 900)?;
            emit_plan(cli, &plan)?;
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
            ensure_redaction_bypass_allowed(*no_redact)?;
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
        MailAction::Export {
            output,
            account,
            mailbox,
            query,
            unread,
            limit,
            no_redact,
        } => {
            ensure_redaction_bypass_allowed(*no_redact)?;
            let mut list = adapter.list(
                profile_name,
                profile,
                &ListMailQuery {
                    account: account.clone(),
                    mailbox: Some(mailbox.clone()),
                    unread_only: *unread,
                    query: query.clone(),
                    limit: usize::MAX,
                    scan_limit: usize::MAX,
                },
            )?;
            if *limit > 0 {
                list.messages.truncate(*limit);
            }
            fs::create_dir_all(output)?;
            let mut exported_files = Vec::new();
            for summary in &list.messages {
                let detail = adapter.read(
                    profile_name,
                    profile,
                    &summary.local_id,
                    Some(&summary.account),
                    Some(&summary.mailbox),
                    *no_redact,
                )?;
                let date_prefix = detail
                    .date_received
                    .as_deref()
                    .and_then(|s| s.get(..10))
                    .unwrap_or("undated");
                let fname = format!(
                    "{}-{}-{}.md",
                    date_prefix,
                    detail.local_id,
                    sanitize_filename(&detail.subject)
                );
                let file_path = output.join(fname);
                let md = format!(
                    "---\nsubject: {:?}\nfrom: {:?}\nto: {:?}\ncc: {:?}\ndate_received: {:?}\naccount: {:?}\nmailbox: {:?}\nlocal_id: {:?}\ntrust_level: \"untrusted_external\"\n---\n\n{}\n",
                    detail.subject,
                    detail.sender,
                    detail.to,
                    detail.cc,
                    detail.date_received.clone().unwrap_or_default(),
                    detail.account,
                    detail.mailbox,
                    detail.local_id,
                    detail.body_text
                );
                fs::write(&file_path, md)?;
                exported_files.push(file_path.display().to_string());
            }
            match cli.format {
                OutputFormat::Pretty => {
                    println!(
                        "{} E-Mail(s) als Markdown exportiert nach: {}",
                        exported_files.len(),
                        output.display()
                    );
                }
                _ => emit_json(
                    cli.format,
                    &serde_json::json!({
                        "ok": true,
                        "output_dir": output.display().to_string(),
                        "exported_count": exported_files.len(),
                        "files": exported_files,
                    }),
                )?,
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
            emit_plan(cli, &plan)?;
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
                _ => {
                    let plans: Vec<_> = plans.iter().map(|p| p.for_output()).collect();
                    emit_json(cli.format, &plans)?
                }
            }
            Ok(())
        }
        PlanAction::Inspect { plan_id } => {
            let plan = store.load(plan_id)?;
            emit_plan(cli, &plan)?;
            Ok(())
        }
        PlanAction::Approve {
            plan_id,
            no_apply,
            details,
        } => {
            if !io::stdin().is_terminal() {
                return Err(MacflyError::ApprovalError(
                    "Plan-Freigabe erfordert immer ein interaktives Terminal (Schutz gegen Self-Approval durch Agenten oder Skripte)."
                        .to_string(),
                ));
            }
            let plan = store.load(plan_id)?;
            let lang = Lang::detect();
            ui::render_plan_for_approval(&plan, cli.color.into(), lang, *details);
            print!(
                "\n{}",
                match lang {
                    Lang::De =>
                        format!("Zur Freigabe Prüfcode '{}' eingeben: ", plan.approval_code),
                    Lang::En => format!(
                        "Type the confirmation code '{}' to approve: ",
                        plan.approval_code
                    ),
                }
            );
            io::stdout().flush()?;
            let mut answer = String::new();
            io::stdin().read_line(&mut answer)?;
            let approved = store.approve(plan_id, answer.trim())?;
            if *no_apply {
                match cli.format {
                    OutputFormat::Pretty => {
                        println!(
                            "{}\n  macfly plan apply {}",
                            lang.t("Freigegeben. Ausführen mit:", "Approved. Run it with:"),
                            approved.plan_id
                        );
                    }
                    _ => emit_json(cli.format, &approved.for_output())?,
                }
                return Ok(());
            }
            if matches!(cli.format, OutputFormat::Pretty) {
                println!(
                    "\n{}\n",
                    lang.t("Freigegeben, wird ausgeführt …", "Approved, running …")
                );
            }
            run_apply(
                cli,
                &store,
                cfg.script_timeout_secs,
                profile_name,
                profile,
                plan_id,
                false,
                *details,
            )
        }
        PlanAction::Apply {
            plan_id,
            resume,
            details,
        } => run_apply(
            cli,
            &store,
            cfg.script_timeout_secs,
            profile_name,
            profile,
            plan_id,
            *resume,
            *details,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_apply(
    cli: &Cli,
    store: &PlanStore,
    timeout_secs: u64,
    profile_name: &str,
    profile: &macfly_core::config::ProfileConfig,
    plan_id: &str,
    resume: bool,
    details: bool,
) -> Result<(), MacflyError> {
    let journal = AuditJournal::default_journal();
    let report = apply_approved_plan(
        store,
        &journal,
        plan_id,
        profile_name,
        profile,
        timeout_secs,
        resume,
    )?;
    match cli.format {
        OutputFormat::Pretty => {
            let plan = store.load(plan_id)?;
            ui::render_apply_report(&report, &plan, cli.color.into(), Lang::detect(), details);
        }
        _ => emit_json(cli.format, &report)?,
    }
    Ok(())
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
        CalendarAction::Export {
            output,
            days,
            calendar,
        } => {
            let res = adapter.events(*days, calendar.as_deref(), 0)?;
            if let Some(parent) = output.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent)?;
            }
            let mut md = format!(
                "---\ntype: \"calendar.export\"\ndays: {}\ncount: {}\n---\n\n# Calendar Events (next {} days)\n\n",
                res.days,
                res.events.len(),
                res.days
            );
            for ev in &res.events {
                let start = ev.start_at.as_deref().unwrap_or("?");
                let loc = ev
                    .location
                    .as_deref()
                    .map(|l| format!(" — *{l}*"))
                    .unwrap_or_default();
                md.push_str(&format!(
                    "- **{}** `[{}]` {}{}\n",
                    start, ev.calendar, ev.title, loc
                ));
            }
            fs::write(output, md)?;
            match cli.format {
                OutputFormat::Pretty => println!(
                    "{} Termin(e) als Markdown exportiert nach: {}",
                    res.events.len(),
                    output.display()
                ),
                _ => emit_json(
                    cli.format,
                    &serde_json::json!({
                        "ok": true,
                        "output": output.display().to_string(),
                        "exported_count": res.events.len(),
                    }),
                )?,
            }
            Ok(())
        }
    }
}

fn handle_reminders(cli: &Cli, action: &RemindersAction) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let (profile_name, profile) = cfg.profile(cli.profile.as_deref())?;
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
        RemindersAction::Export {
            output,
            list,
            overdue,
        } => {
            let res = adapter.list(list.as_deref(), *overdue, 0)?;
            if let Some(parent) = output.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent)?;
            }
            let mut md = format!(
                "---\ntype: \"reminders.export\"\ncount: {}\n---\n\n# Open Reminders\n\n",
                res.reminders.len()
            );
            for r in &res.reminders {
                let due = r
                    .due_at
                    .as_deref()
                    .and_then(|s| s.get(..10))
                    .map(|d| format!(" *(due: {d})*"))
                    .unwrap_or_default();
                md.push_str(&format!("- [ ] `[{}]` {}{}\n", r.list, r.title, due));
            }
            fs::write(output, md)?;
            match cli.format {
                OutputFormat::Pretty => println!(
                    "{} Erinnerung(en) als Markdown-Checkliste exportiert nach: {}",
                    res.reminders.len(),
                    output.display()
                ),
                _ => emit_json(
                    cli.format,
                    &serde_json::json!({
                        "ok": true,
                        "output": output.display().to_string(),
                        "exported_count": res.reminders.len(),
                    }),
                )?,
            }
            Ok(())
        }
        RemindersAction::Import { input, list } => {
            let files = collect_import_files(input, false, is_supported_reminder_file)?;
            if files.is_empty() {
                return Err(MacflyError::PlanError(format!(
                    "Keine unterstützten Dateien (.md, .txt, .json, .jsonl, .csv) gefunden unter {}",
                    input.display()
                )));
            }
            let mut operations = Vec::new();
            for path in &files {
                let raw = fs::read_to_string(path)?;
                let items = parse_reminders_from_str(path, &raw)?;
                for item in items {
                    operations.push(PlanOperation::RemindersCreate {
                        list: item.list_override.or_else(|| list.clone()),
                        title: item.title,
                        notes: item.notes,
                    });
                }
            }
            if operations.is_empty() {
                return Err(MacflyError::PlanError(format!(
                    "Keine importierbaren Aufgaben gefunden in {}",
                    input.display()
                )));
            }
            let store = PlanStore::default_store();
            let proposal = PlanProposal {
                schema_version: "1.0".to_string(),
                summary: Some(format!(
                    "Reminders-Import: {} Aufgabe(n) aus {}",
                    operations.len(),
                    input.display()
                )),
                operations,
            };
            let plan = store.create(profile_name, profile, proposal, 900)?;
            emit_plan(cli, &plan)?;
            Ok(())
        }
    }
}

fn handle_music(cli: &Cli, action: &MusicAction) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let (profile_name, profile) = cfg.profile(cli.profile.as_deref())?;
    let adapter = MusicAdapter::new(cfg.script_timeout_secs);
    match action {
        MusicAction::Playlists => {
            let res = adapter.playlists()?;
            match cli.format {
                OutputFormat::Pretty => ui::render_music_playlists(&res, cli.color.into()),
                _ => emit_json(cli.format, &res)?,
            }
            Ok(())
        }
        MusicAction::Tracks { playlist, limit } => {
            let res = adapter.tracks(playlist.as_deref(), *limit)?;
            match cli.format {
                OutputFormat::Pretty => ui::render_music_tracks(&res, cli.color.into()),
                _ => emit_json(cli.format, &res)?,
            }
            Ok(())
        }
        MusicAction::Stats {
            top,
            stale_days,
            csv,
        } => {
            let res = adapter.stats(*top, *stale_days)?;
            if let Some(path) = csv {
                write_music_csv(path, &res.all)?;
            }
            match cli.format {
                OutputFormat::Pretty => ui::render_music_stats(&res, *stale_days, cli.color.into()),
                _ => emit_json(cli.format, &res)?,
            }
            Ok(())
        }
        MusicAction::Add {
            playlist,
            track_ids,
        } => {
            let store = PlanStore::default_store();
            let proposal = PlanProposal {
                schema_version: "1.0".to_string(),
                summary: Some(format!(
                    "{} Titel zur Playlist '{playlist}'",
                    track_ids.len()
                )),
                operations: vec![PlanOperation::MusicPlaylistAdd {
                    playlist: playlist.clone(),
                    track_ids: track_ids.clone(),
                }],
            };
            let plan = store.create(profile_name, profile, proposal, 900)?;
            emit_plan(cli, &plan)?;
            Ok(())
        }
    }
}

fn write_music_csv(
    path: &std::path::Path,
    tracks: &[macfly_adapters::music::MusicTrackStat],
) -> Result<(), MacflyError> {
    fn q(s: &str) -> String {
        format!("\"{}\"", s.replace('"', "\"\""))
    }
    let mut out = String::from(
        "id,title,artist,album,genre,played_count,skipped_count,last_played,added_at,rating,duration_secs\n",
    );
    for t in tracks {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            t.id,
            q(&t.title),
            q(&t.artist),
            q(&t.album),
            q(&t.genre),
            t.played_count,
            t.skipped_count,
            t.last_played.as_deref().unwrap_or(""),
            t.added_at.as_deref().unwrap_or(""),
            t.rating,
            t.duration_secs
        ));
    }
    fs::write(path, out)?;
    Ok(())
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

fn handle_contacts(cli: &Cli, action: &ContactsAction) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let adapter = ContactsAdapter::new(cfg.script_timeout_secs);
    match action {
        ContactsAction::List { limit } => {
            let res = adapter.list(None, *limit)?;
            print_contacts_list(cli.format, &res)
        }
        ContactsAction::Search { query, limit } => {
            let res = adapter.list(Some(query), *limit)?;
            print_contacts_list(cli.format, &res)
        }
        ContactsAction::Audit { details } => {
            let rep = adapter.audit()?;
            match cli.format {
                OutputFormat::Pretty => ui::render_contacts_audit(&rep, *details, cli.color.into()),
                _ => emit_json(cli.format, &rep)?,
            }
            Ok(())
        }
        ContactsAction::Export {
            output,
            query,
            limit,
        } => {
            let res = adapter.list(query.as_deref(), *limit)?;
            fs::create_dir_all(output)?;
            let mut exported_files = Vec::new();
            let mut used_paths = std::collections::HashSet::new();
            for c in &res.contacts {
                let fname = format!("{}.md", sanitize_filename(&c.name));
                let file_path = output.join(fname);
                let file_path = unique_export_path(file_path, &mut used_paths);
                let md = format!(
                    "---\nname: {:?}\norganization: {:?}\nemails: {:?}\nphones: {:?}\nid: {:?}\n---\n\n# {}\n",
                    c.name,
                    c.organization.clone().unwrap_or_default(),
                    c.emails,
                    c.phones,
                    c.id,
                    c.name
                );
                fs::write(&file_path, md)?;
                exported_files.push(file_path.display().to_string());
            }
            match cli.format {
                OutputFormat::Pretty => {
                    println!(
                        "{} Kontakt(e) als Markdown exportiert nach: {}",
                        exported_files.len(),
                        output.display()
                    );
                }
                _ => emit_json(
                    cli.format,
                    &serde_json::json!({
                        "ok": true,
                        "output_dir": output.display().to_string(),
                        "exported_count": exported_files.len(),
                        "files": exported_files,
                    }),
                )?,
            }
            Ok(())
        }
    }
}

fn print_contacts_list(
    format: OutputFormat,
    res: &macfly_core::model::ContactsListResult,
) -> Result<(), MacflyError> {
    match format {
        OutputFormat::Pretty => {
            println!(
                "Kontakte: {} angezeigt (von {} Treffern)",
                res.returned, res.total_matched
            );
            println!();
            for c in &res.contacts {
                let org = c
                    .organization
                    .as_deref()
                    .map(|o| format!(" ({o})"))
                    .unwrap_or_default();
                let mail = c.emails.first().map(String::as_str).unwrap_or("—");
                let phone = c.phones.first().map(String::as_str).unwrap_or("—");
                println!("  • {}{}  |  {}  |  {}", c.name, org, mail, phone);
            }
        }
        _ => emit_json(format, res)?,
    }
    Ok(())
}

fn handle_context(cli: &Cli, action: &ContextAction) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let (profile_name, profile) = cfg.profile(cli.profile.as_deref())?;

    match action {
        ContextAction::DailyBrief {
            mail_limit,
            notes_limit,
        } => {
            let brief = build_daily_brief(
                profile_name,
                profile,
                cfg.script_timeout_secs,
                *mail_limit,
                *notes_limit,
            );
            match cli.format {
                OutputFormat::Pretty => ui::render_daily_brief(&brief, cli.color.into()),
                _ => emit_json(cli.format, &brief)?,
            }
            Ok(())
        }
        ContextAction::Person { query, limit } => {
            let ctx = build_person_context(
                profile_name,
                profile,
                cfg.script_timeout_secs,
                query,
                *limit,
            );
            match cli.format {
                OutputFormat::Pretty => ui::render_person_context(&ctx, cli.color.into()),
                _ => emit_json(cli.format, &ctx)?,
            }
            Ok(())
        }
        ContextAction::MeetingPrep {
            query,
            create_note,
            folder,
        } => {
            let prep = build_meeting_prep(
                profile_name,
                profile,
                cfg.script_timeout_secs,
                query.as_deref(),
            );
            if *create_note {
                let title = format!("Meeting Prep: {}", prep.query);
                let mut lines = Vec::new();
                if let Some(ev) = &prep.target_event {
                    lines.push(format!(
                        "Termin: {} ({})",
                        ev.title,
                        ev.start_at.as_deref().unwrap_or("?")
                    ));
                }
                if !prep.related_contacts.is_empty() {
                    lines.push("\nKontakte:".to_string());
                    for c in &prep.related_contacts {
                        lines.push(format!("• {} ({})", c.name, c.emails.join(", ")));
                    }
                }
                if !prep.related_mails.is_empty() {
                    lines.push("\nLetzte E-Mails:".to_string());
                    for m in &prep.related_mails {
                        lines.push(format!("• {} — {}", m.subject, m.sender));
                    }
                }
                if !prep.related_notes.is_empty() {
                    lines.push("\nVerwandte Notizen:".to_string());
                    for n in &prep.related_notes {
                        lines.push(format!("• [{}] {}", n.folder_path, n.title));
                    }
                }
                let store = PlanStore::default_store();
                let proposal = PlanProposal {
                    schema_version: "1.0".to_string(),
                    summary: Some(format!("Meeting-Prep-Notiz '{}' anlegen", title)),
                    operations: vec![PlanOperation::NotesCreate {
                        account: None,
                        folder: folder.clone(),
                        title,
                        body: lines.join("\n"),
                    }],
                };
                let plan = store.create(profile_name, profile, proposal, 900)?;
                emit_plan(cli, &plan)?;
            } else {
                match cli.format {
                    OutputFormat::Pretty => ui::render_meeting_prep(&prep, cli.color.into()),
                    _ => emit_json(cli.format, &prep)?,
                }
            }
            Ok(())
        }
        ContextAction::WeeklyReview => {
            let rev = build_weekly_review(profile_name, profile, cfg.script_timeout_secs);
            match cli.format {
                OutputFormat::Pretty => ui::render_weekly_review(&rev, cli.color.into()),
                _ => emit_json(cli.format, &rev)?,
            }
            Ok(())
        }
    }
}

fn handle_report(cli: &Cli, action: &ReportAction) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let (profile_name, profile) = cfg.profile(cli.profile.as_deref())?;

    let (output, typst_src, md_src) = match action {
        ReportAction::Person { query, output } => {
            let ctx =
                build_person_context(profile_name, profile, cfg.script_timeout_secs, query, 15);
            (
                output,
                person_context_to_typst(&ctx),
                Some(person_context_to_markdown(&ctx)),
            )
        }
        ReportAction::DailyBrief { output } => {
            let brief = build_daily_brief(profile_name, profile, cfg.script_timeout_secs, 15, 15);
            (output, daily_brief_to_typst(&brief), None)
        }
        ReportAction::WeeklyReview { output } => {
            let rev = build_weekly_review(profile_name, profile, cfg.script_timeout_secs);
            (output, weekly_review_to_typst(&rev), None)
        }
    };

    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let ext = output
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    if ext == "md" {
        let content = md_src.unwrap_or(typst_src);
        fs::write(output, content)?;
    } else if ext == "pdf" {
        let typ_path = output.with_extension("typ");
        fs::write(&typ_path, &typst_src)?;
        let status = Command::new("typst")
            .arg("compile")
            .arg(&typ_path)
            .arg(output)
            .status()
            .map_err(|e| {
                MacflyError::ScriptError(format!(
                    "Typst-Quelltext wurde unter {} gespeichert, aber `typst compile` konnte nicht ausgeführt werden ({e}). Nutze `.typ` für renderreport/typst.",
                    typ_path.display()
                ))
            })?;
        if !status.success() {
            return Err(MacflyError::ScriptError(format!(
                "`typst compile` fehlgeschlagen (Status {status})"
            )));
        }
    } else {
        fs::write(output, typst_src)?;
    }

    match cli.format {
        OutputFormat::Pretty => println!("Dossier/Report geschrieben nach: {}", output.display()),
        _ => emit_json(
            cli.format,
            &serde_json::json!({ "ok": true, "output": output.display().to_string() }),
        )?,
    }
    Ok(())
}

fn handle_skill(cli: &Cli, action: &SkillAction) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let (profile_name, _) = cfg.profile(cli.profile.as_deref())?;
    match action {
        SkillAction::Show => {
            print!("{}", render_agent_skill_markdown(profile_name));
            Ok(())
        }
        SkillAction::Export { output } => {
            let written = export_skill_file(output, profile_name)?;
            match cli.format {
                OutputFormat::Pretty => {
                    println!("Agent-Skill exportiert nach: {}", written.display())
                }
                _ => emit_json(
                    cli.format,
                    &serde_json::json!({ "ok": true, "path": written.display().to_string() }),
                )?,
            }
            Ok(())
        }
    }
}

fn handle_schedule(cli: &Cli, action: &ScheduleAction) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let (profile_name, _) = cfg.profile(cli.profile.as_deref())?;

    match action {
        ScheduleAction::List => {
            let jobs = list_installed_jobs()?;
            match cli.format {
                OutputFormat::Pretty => {
                    if jobs.is_empty() {
                        println!("Keine MacFly LaunchAgents installiert.");
                    } else {
                        println!("Installierte MacFly LaunchAgents ({}):", jobs.len());
                        for j in &jobs {
                            println!("  • {} ({}) → {}", j.name, j.label, j.plist_path);
                        }
                    }
                }
                _ => emit_json(cli.format, &jobs)?,
            }
            Ok(())
        }
        ScheduleAction::Install {
            name,
            task,
            hour,
            minute,
            dry_run,
        } => {
            if *hour > 23 || *minute > 59 {
                return Err(MacflyError::PlanError(
                    "Ungültige Uhrzeit (hour 0..23, minute 0..59)".to_string(),
                ));
            }
            let label = job_label(name);
            let bin_path = std::env::current_exe()
                .unwrap_or_else(|_| PathBuf::from("macfly"))
                .display()
                .to_string();
            let args: Vec<String> = match task {
                ScheduleTask::DailyBrief => vec![
                    "--profile".to_string(),
                    profile_name.to_string(),
                    "--format".to_string(),
                    "json".to_string(),
                    "context".to_string(),
                    "daily-brief".to_string(),
                ],
                ScheduleTask::WeeklyReview => vec![
                    "--profile".to_string(),
                    profile_name.to_string(),
                    "--format".to_string(),
                    "json".to_string(),
                    "context".to_string(),
                    "weekly-review".to_string(),
                ],
                ScheduleTask::NotesAudit => vec![
                    "--profile".to_string(),
                    profile_name.to_string(),
                    "--format".to_string(),
                    "json".to_string(),
                    "notes".to_string(),
                    "audit".to_string(),
                ],
            };
            let log_dir = data_dir().join("logs");
            fs::create_dir_all(&log_dir)?;
            let xml = render_launchd_plist(
                &label,
                &bin_path,
                &args,
                *hour,
                *minute,
                &log_dir.display().to_string(),
            );

            if *dry_run {
                print!("{xml}");
                return Ok(());
            }

            let agents_dir = launch_agents_dir()?;
            fs::create_dir_all(&agents_dir)?;
            let plist_path = agents_dir.join(format!("{label}.plist"));
            fs::write(&plist_path, xml)?;
            let _ = Command::new("launchctl")
                .arg("unload")
                .arg(&plist_path)
                .output();
            let _ = Command::new("launchctl")
                .arg("load")
                .arg(&plist_path)
                .output();

            match cli.format {
                OutputFormat::Pretty => println!(
                    "LaunchAgent '{}' installiert ({:02}:{:02} Uhr): {}",
                    label,
                    hour,
                    minute,
                    plist_path.display()
                ),
                _ => emit_json(
                    cli.format,
                    &serde_json::json!({
                        "ok": true,
                        "label": label,
                        "plist_path": plist_path.display().to_string(),
                    }),
                )?,
            }
            Ok(())
        }
        ScheduleAction::Remove { name } => {
            let label = job_label(name);
            let plist_path = launch_agents_dir()?.join(format!("{label}.plist"));
            if !plist_path.exists() {
                return Err(MacflyError::NotFound(format!(
                    "LaunchAgent '{}' nicht gefunden",
                    plist_path.display()
                )));
            }
            let _ = Command::new("launchctl")
                .arg("unload")
                .arg(&plist_path)
                .output();
            fs::remove_file(&plist_path)?;
            match cli.format {
                OutputFormat::Pretty => println!("LaunchAgent '{}' entfernt.", label),
                _ => emit_json(
                    cli.format,
                    &serde_json::json!({ "ok": true, "removed": label }),
                )?,
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
                println!(
                    "              von: {}  |  local_id: {}",
                    m.sender, m.local_id
                );
            }
        }
        _ => emit_json(format, res)?,
    }
    Ok(())
}

fn unique_export_path(path: PathBuf, used: &mut std::collections::HashSet<String>) -> PathBuf {
    let mut candidate = path.clone();
    let mut suffix = 2;
    while !used.insert(candidate.to_string_lossy().to_lowercase()) {
        let stem = path.file_stem().unwrap().to_string_lossy();
        candidate = path.with_file_name(format!("{stem}-{suffix}.md"));
        suffix += 1;
    }
    candidate
}

fn sanitize_filename(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\n' | '\r' => '-',
            _ => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.');
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed.chars().take(80).collect()
    }
}

fn collect_import_files(
    input: &std::path::Path,
    recursive: bool,
    predicate: fn(&std::path::Path) -> bool,
) -> Result<Vec<PathBuf>, MacflyError> {
    if input.is_file() {
        return Ok(vec![input.to_path_buf()]);
    }
    if !input.is_dir() {
        return Err(MacflyError::NotFound(format!(
            "Import-Pfad existiert nicht: {}",
            input.display()
        )));
    }
    let mut out = Vec::new();
    collect_files_rec(input, recursive, predicate, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_files_rec(
    dir: &std::path::Path,
    recursive: bool,
    predicate: fn(&std::path::Path) -> bool,
    out: &mut Vec<PathBuf>,
) -> Result<(), MacflyError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && predicate(&path) {
            out.push(path);
        } else if recursive && path.is_dir() {
            collect_files_rec(&path, true, predicate, out)?;
        }
    }
    Ok(())
}

/// Datum im Format JJJJ-MM-TT (nur die Form wird geprüft; Photos liest es als Datum).
fn validate_since(since: Option<&str>) -> Result<(), MacflyError> {
    if let Some(d) = since {
        let b = d.as_bytes();
        let ok = b.len() == 10
            && b.iter().enumerate().all(|(i, c)| match i {
                4 | 7 => *c == b'-',
                _ => c.is_ascii_digit(),
            });
        if !ok {
            return Err(MacflyError::PlanError(format!(
                "`--since` erwartet JJJJ-MM-TT, bekam '{d}'"
            )));
        }
    }
    Ok(())
}

fn handle_photos(cli: &Cli, action: &PhotosAction) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let (profile_name, profile) = cfg.profile(cli.profile.as_deref())?;
    let adapter = PhotosAdapter::new(cfg.script_timeout_secs);

    match action {
        PhotosAction::Albums => {
            let res = adapter.albums()?;
            match cli.format {
                OutputFormat::Pretty => {
                    println!("Alben ({}):", res.total_albums);
                    for a in &res.albums {
                        println!("  {} ({})", a.name, a.count);
                    }
                }
                _ => emit_json(cli.format, &res)?,
            }
            Ok(())
        }
        PhotosAction::List {
            album,
            keyword,
            since,
            limit,
        } => {
            validate_since(since.as_deref())?;
            let res = adapter.list(
                profile,
                album.as_deref(),
                keyword.as_deref(),
                since.as_deref(),
                *limit,
            )?;
            match cli.format {
                OutputFormat::Pretty => {
                    println!("{} von {} Treffern:", res.returned, res.total_matched);
                    for p in &res.items {
                        println!(
                            "  {}  {}  {}{}",
                            p.date.as_deref().unwrap_or("(ohne Datum)"),
                            p.filename,
                            p.title.as_deref().unwrap_or(""),
                            if p.keywords.is_empty() {
                                String::new()
                            } else {
                                format!("  [{}]", p.keywords.join(", "))
                            }
                        );
                    }
                }
                _ => emit_json(cli.format, &res)?,
            }
            Ok(())
        }
        PhotosAction::Export {
            to,
            album,
            keyword,
            since,
            ids,
            limit,
            originals,
            sidecar,
            convert,
            max_size,
            quality,
        } => {
            validate_since(since.as_deref())?;
            if convert.is_none() && (max_size.is_some() || quality.is_some()) {
                return Err(MacflyError::PlanError(
                    "`--max-size` und `--quality` brauchen `--convert`".to_string(),
                ));
            }
            let (media_ids, label) = if !ids.is_empty() {
                (ids.clone(), Lang::detect().t("Einzelauswahl", "Selection"))
            } else {
                if album.is_none() && keyword.is_none() && since.is_none() {
                    return Err(MacflyError::PlanError(
                        "Bitte die Auswahl eingrenzen: `--album`, `--keyword`, `--since` oder `--id` (kein versehentlicher Export der ganzen Mediathek)."
                            .to_string(),
                    ));
                }
                let res = adapter.list(
                    profile,
                    album.as_deref(),
                    keyword.as_deref(),
                    since.as_deref(),
                    *limit,
                )?;
                if res.items.is_empty() {
                    return Err(MacflyError::PlanError(
                        "Keine passenden Fotos gefunden".to_string(),
                    ));
                }
                let label = match album {
                    Some(a) => format!("Album '{a}'"),
                    None => Lang::detect().t("Mediathek, gefiltert", "filtered library"),
                };
                (res.items.into_iter().map(|i| i.id).collect(), label)
            };

            let count = media_ids.len();
            let store = PlanStore::default_store();
            let proposal = PlanProposal {
                schema_version: "1.0".to_string(),
                summary: Some(match Lang::detect() {
                    Lang::De => format!("{count} Foto(s) exportieren nach '{to}'"),
                    Lang::En => format!("Export {count} photo(s) to '{to}'"),
                }),
                operations: vec![PlanOperation::PhotosExport {
                    media_ids,
                    destination: to.clone(),
                    originals: *originals,
                    sidecar: (*sidecar).into(),
                    convert: convert.map(|c| PhotoConvert {
                        format: c.into(),
                        max_size: *max_size,
                        quality: *quality,
                    }),
                    label: Some(label),
                }],
            };
            let plan = store.create(profile_name, profile, proposal, 900)?;
            emit_plan(cli, &plan)?;
            Ok(())
        }
    }
}

fn handle_safari(cli: &Cli, action: &SafariAction) -> Result<(), MacflyError> {
    let (cfg, _, _) = Config::load_or_default(cli.config.as_deref())?;
    let (profile_name, profile) = cfg.profile(cli.profile.as_deref())?;
    let adapter = SafariAdapter::new(cfg.script_timeout_secs);

    match action {
        SafariAction::Tabs { query } => {
            let res = adapter.tabs(query.as_deref())?;
            match cli.format {
                OutputFormat::Pretty => {
                    println!("Offene Safari-Tabs ({}):", res.total_tabs);
                    for t in &res.tabs {
                        println!(
                            "  [W{}:T{}] {} — {}",
                            t.window_index, t.tab_index, t.title, t.url
                        );
                    }
                }
                _ => emit_json(cli.format, &res)?,
            }
            Ok(())
        }
        SafariAction::Export { output, query } => {
            let res = adapter.tabs(query.as_deref())?;
            if let Some(parent) = output.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent)?;
            }
            let mut md = format!(
                "---\ntype: \"safari.tabs\"\ncount: {}\n---\n\n# Safari Tabs\n\n",
                res.total_tabs
            );
            for t in &res.tabs {
                md.push_str(&format!("- [{}]({})\n", t.title, t.url));
            }
            fs::write(output, md)?;
            match cli.format {
                OutputFormat::Pretty => println!(
                    "{} Safari-Tab(s) als Markdown exportiert nach: {}",
                    res.total_tabs,
                    output.display()
                ),
                _ => emit_json(
                    cli.format,
                    &serde_json::json!({
                        "ok": true,
                        "output": output.display().to_string(),
                        "exported_count": res.total_tabs,
                    }),
                )?,
            }
            Ok(())
        }
        SafariAction::ToNote {
            folder,
            title,
            query,
        } => {
            let res = adapter.tabs(query.as_deref())?;
            if res.tabs.is_empty() {
                return Err(MacflyError::PlanError(
                    "Keine passenden offenen Safari-Tabs gefunden".to_string(),
                ));
            }
            let note_title = title
                .clone()
                .unwrap_or_else(|| format!("Safari Tabs ({} Links)", res.total_tabs));
            let body: String = res
                .tabs
                .iter()
                .map(|t| format!("• {} — {}", t.title, t.url))
                .collect::<Vec<_>>()
                .join("\n");

            let store = PlanStore::default_store();
            let proposal = PlanProposal {
                schema_version: "1.0".to_string(),
                summary: Some(format!(
                    "Safari-Tabs ({}) als Notiz nach '{}' sichern",
                    res.total_tabs, folder
                )),
                operations: vec![PlanOperation::NotesCreate {
                    account: None,
                    folder: folder.clone(),
                    title: note_title,
                    body,
                }],
            };
            let plan = store.create(profile_name, profile, proposal, 900)?;
            emit_plan(cli, &plan)?;
            Ok(())
        }
    }
}
