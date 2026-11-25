use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config::{Preset, ProfileConfig, data_dir};
use crate::error::MacflyError;
use crate::model::compute_sha256_hex;

/// Eingabeformat, mit dem ein Mensch oder KI-Agent einen Änderungsplan vorschlägt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanProposal {
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub summary: Option<String>,
    pub operations: Vec<PlanOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PlanOperation {
    NotesCreate {
        account: Option<String>,
        folder: String,
        title: String,
        body: String,
    },
    NotesAppend {
        note_id: String,
        #[serde(default)]
        expected_revision: Option<String>,
        text: String,
    },
    NotesMove {
        note_id: String,
        #[serde(default)]
        expected_revision: Option<String>,
        destination_folder: String,
        #[serde(default)]
        destination_account: Option<String>,
    },
    NotesMoveFolderContents {
        source_folder_id: String,
        source_folder_path: String,
        destination_folder_path: String,
        #[serde(default)]
        destination_account: Option<String>,
    },
    NotesCreateFolder {
        account: Option<String>,
        name: String,
        #[serde(default)]
        parent_folder: Option<String>,
    },
    NotesDeleteEmptyFolder {
        folder_id: String,
        folder_name: String,
    },
    MailCreateDraft {
        #[serde(default)]
        account: Option<String>,
        to: Vec<String>,
        #[serde(default)]
        cc: Vec<String>,
        subject: String,
        body: String,
        /// Optional: Name einer Signatur in Apple Mail (sonst aus config.json).
        #[serde(default)]
        signature_name: Option<String>,
        /// Optional: Bezugs-Mail-ID (für Antwort-Kontext im Entwurf).
        #[serde(default)]
        reply_to_local_id: Option<String>,
    },
    MailMarkRead {
        local_id: String,
        account: String,
        mailbox: String,
        read: bool,
    },
    MailMove {
        local_id: String,
        account: String,
        from_mailbox: String,
        to_mailbox: String,
    },
    ShortcutsRun {
        name: String,
        #[serde(default)]
        input: Option<String>,
    },
    RemindersCreate {
        #[serde(default)]
        list: Option<String>,
        title: String,
        #[serde(default)]
        notes: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reversibility {
    Reversible,
    BestEffort,
    Irreversible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    PendingApproval,
    Approved,
    PartiallyApplied,
    Applied,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationRecord {
    pub index: usize,
    pub succeeded: bool,
    pub message: String,
    pub executed_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredPlan {
    pub schema_version: String,
    pub plan_id: String,
    pub profile: String,
    pub summary: String,
    pub created_at_unix: u64,
    pub expires_at_unix: u64,
    pub status: PlanStatus,
    pub risk_level: RiskLevel,
    pub reversibility: Reversibility,
    pub plan_hash: String,
    /// Zufälliger Prüfcode für die menschliche Freigabe. Wird nie an Agenten ausgegeben (siehe `for_output`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub approval_code: String,
    #[serde(default)]
    pub approved_at_unix: Option<u64>,
    /// Wer freigegeben hat: `human` oder `preset:<name>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<String>,
    #[serde(default)]
    pub applied_at_unix: Option<u64>,
    pub operations: Vec<PlanOperation>,
    pub preview_lines: Vec<String>,
    #[serde(default)]
    pub completed_ops: Vec<OperationRecord>,
}

impl StoredPlan {
    /// Kopie für Ausgaben an Terminal, JSON oder MCP — ohne Prüfcode.
    pub fn for_output(&self) -> Self {
        Self {
            approval_code: String::new(),
            ..self.clone()
        }
    }
}

/// Aktionen mit ihrer Risikostufe — Grundlage für `macfly policy` und die Voreinstellungen.
pub const ACTION_RISKS: &[(&str, RiskLevel)] = &[
    ("notes_create", RiskLevel::Low),
    ("notes_create_folder", RiskLevel::Low),
    ("mail_create_draft", RiskLevel::Low),
    ("mail_mark_read", RiskLevel::Low),
    ("reminders_create", RiskLevel::Low),
    ("notes_append", RiskLevel::Medium),
    ("notes_move", RiskLevel::Medium),
    ("notes_move_folder_contents", RiskLevel::Medium),
    ("notes_delete_empty_folder", RiskLevel::Medium),
    ("mail_move", RiskLevel::Medium),
    ("shortcuts_run", RiskLevel::High),
];

/// Ob eine einzelne Aktion dieser Risikostufe unter der Voreinstellung automatisch laufen darf.
pub fn preset_allows(preset: Preset, risk: RiskLevel) -> bool {
    match preset {
        Preset::Strict => false,
        Preset::Balanced => risk == RiskLevel::Low,
        Preset::Open => risk <= RiskLevel::Medium,
    }
}

/// Ob ein ganzer Plan unter der Voreinstellung ohne menschliche Freigabe laufen darf.
/// `balanced` nutzt das Plan-Risiko (mehr als 5 Operationen gelten als hoch),
/// `open` prüft nur die einzelnen Operationen.
pub fn preset_auto_approves(preset: Preset, plan_risk: RiskLevel, ops: &[PlanOperation]) -> bool {
    match preset {
        Preset::Strict => false,
        Preset::Balanced => preset_allows(preset, plan_risk),
        Preset::Open => ops.iter().all(|o| preset_allows(preset, o.risk_level())),
    }
}

/// Exklusiver Dateilock gegen parallele oder doppelte Ausführung desselben Plans.
pub struct PlanLock {
    lock_path: PathBuf,
}

impl Drop for PlanLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

impl PlanOperation {
    pub fn risk_level(&self) -> RiskLevel {
        match self {
            Self::NotesCreate { .. }
            | Self::NotesCreateFolder { .. }
            | Self::MailCreateDraft { .. }
            | Self::MailMarkRead { .. }
            | Self::RemindersCreate { .. } => RiskLevel::Low,
            Self::NotesAppend { .. }
            | Self::NotesMove { .. }
            | Self::NotesMoveFolderContents { .. }
            | Self::NotesDeleteEmptyFolder { .. }
            | Self::MailMove { .. } => RiskLevel::Medium,
            // Kurzbefehle können beliebige Nebenwirkungen in macOS/externen Diensten auslösen.
            Self::ShortcutsRun { .. } => RiskLevel::High,
        }
    }

    pub fn reversibility(&self) -> Reversibility {
        match self {
            Self::ShortcutsRun { .. } => Reversibility::Irreversible,
            Self::NotesAppend { .. } => Reversibility::BestEffort,
            _ => Reversibility::Reversible,
        }
    }

    /// Vollständige Inhalts- und Diff-Vorschau für die menschliche Prüfung vor Freigabe.
    pub fn detailed_preview(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "{}  [risk: {:?} | reversibility: {:?}]",
            self.preview(),
            self.risk_level(),
            self.reversibility()
        )];
        match self {
            Self::MailCreateDraft {
                account,
                to,
                cc,
                subject,
                body,
                signature_name,
                reply_to_local_id,
            } => {
                if let Some(acc) = account {
                    lines.push(format!("    Account:   {acc}"));
                }
                lines.push(format!("    To:        {}", to.join(", ")));
                if !cc.is_empty() {
                    lines.push(format!("    Cc:        {}", cc.join(", ")));
                }
                lines.push(format!("    Subject:   {subject}"));
                if let Some(sig) = signature_name {
                    lines.push(format!("    Signature: {sig}"));
                }
                if let Some(reply_id) = reply_to_local_id {
                    lines.push(format!("    Reply-To:  {reply_id}"));
                }
                lines.push("    --- Draft body (will NOT be sent automatically) ---".to_string());
                for body_line in body.lines() {
                    lines.push(format!("    | {body_line}"));
                }
                if body.is_empty() {
                    lines.push("    | (leer)".to_string());
                }
            }
            Self::NotesCreate {
                account,
                folder,
                title,
                body,
            } => {
                if let Some(acc) = account {
                    lines.push(format!("    Account: {acc}"));
                }
                lines.push(format!("    Folder:  {folder}"));
                lines.push(format!("    Title:   {title}"));
                lines.push("    --- New note content ---".to_string());
                for body_line in body.lines() {
                    lines.push(format!("    + {body_line}"));
                }
            }
            Self::NotesAppend {
                note_id,
                expected_revision,
                text,
            } => {
                lines.push(format!("    Note ID:  {note_id}"));
                if let Some(rev) = expected_revision {
                    lines.push(format!("    Revision: {rev}"));
                }
                lines.push("    --- Appended text ---".to_string());
                for tline in text.lines() {
                    lines.push(format!("    + {tline}"));
                }
            }
            Self::ShortcutsRun { name, input } => {
                lines.push(format!("    Shortcut: {name}"));
                if let Some(inp) = input {
                    lines.push(format!("    Input:    {inp}"));
                }
                lines.push(
                    "    ! HIGH RISK: Shortcuts can trigger arbitrary macOS or external actions and cannot be rolled back."
                        .to_string(),
                );
            }
            Self::RemindersCreate { list, title, notes } => {
                if let Some(l) = list {
                    lines.push(format!("    List:  {l}"));
                }
                lines.push(format!("    Title: {title}"));
                if let Some(n) = notes {
                    for nline in n.lines() {
                        lines.push(format!("    + {nline}"));
                    }
                }
            }
            _ => {}
        }
        lines
    }

    pub fn preview(&self) -> String {
        match self {
            Self::NotesCreate {
                folder,
                title,
                body,
                ..
            } => format!(
                "[notes.create] Neue Notiz '{}' in Ordner '{}' ({} Zeichen)",
                title,
                folder,
                body.chars().count()
            ),
            Self::NotesAppend { note_id, text, .. } => format!(
                "[notes.append] An Notiz '{}' anhängen ({} Zeichen)",
                note_id,
                text.chars().count()
            ),
            Self::NotesMove {
                note_id,
                destination_folder,
                ..
            } => format!(
                "[notes.move] Notiz '{}' → Ordner '{}'",
                note_id, destination_folder
            ),
            Self::NotesMoveFolderContents {
                source_folder_path,
                destination_folder_path,
                ..
            } => format!(
                "[notes.move_folder_contents] Alle Notizen aus '{}' → '{}'",
                source_folder_path, destination_folder_path
            ),
            Self::NotesCreateFolder {
                name,
                parent_folder,
                ..
            } => match parent_folder {
                Some(p) => format!("[notes.create_folder] Neuer Unterordner '{p}/{name}'"),
                None => format!("[notes.create_folder] Neuer Ordner '{name}'"),
            },
            Self::NotesDeleteEmptyFolder {
                folder_id,
                folder_name,
            } => format!(
                "[notes.delete_empty_folder] Leeren Ordner '{}' ({}) entfernen",
                folder_name, folder_id
            ),
            Self::MailCreateDraft {
                to,
                subject,
                body,
                ..
            } => format!(
                "[mail.create_draft] Entwurf an {} | Betreff: '{}' ({} Zeichen, KEIN Versand)",
                to.join(", "),
                subject,
                body.chars().count()
            ),
            Self::MailMarkRead {
                local_id,
                mailbox,
                read,
                ..
            } => format!(
                "[mail.mark_read] Mail '{}' in '{}' als {} markieren",
                local_id,
                mailbox,
                if *read { "gelesen" } else { "ungelesen" }
            ),
            Self::MailMove {
                local_id,
                from_mailbox,
                to_mailbox,
                ..
            } => format!(
                "[mail.move] Mail '{}' von '{}' → '{}'",
                local_id, from_mailbox, to_mailbox
            ),
            Self::ShortcutsRun { name, input } => match input {
                Some(_) => format!("[shortcuts.run] Kurzbefehl '{name}' (mit Eingabe) ausführen"),
                None => format!("[shortcuts.run] Kurzbefehl '{name}' ausführen"),
            },
            Self::RemindersCreate { list, title, .. } => match list {
                Some(l) => format!("[reminders.create] Neue Erinnerung '{title}' in Liste '{l}'"),
                None => format!("[reminders.create] Neue Erinnerung '{title}'"),
            },
        }
    }

    pub fn validate_against_profile(
        &self,
        profile_name: &str,
        profile: &ProfileConfig,
    ) -> Result<(), MacflyError> {
        match self {
            Self::NotesCreate {
                account, folder, ..
            } => {
                if let Some(acc) = account
                    && !profile.notes.is_account_allowed(acc)
                {
                    return Err(MacflyError::ScopeDenied(format!(
                        "Notes-Account '{acc}' im Profil '{profile_name}' nicht erlaubt"
                    )));
                }
                if !profile.notes.is_folder_allowed(folder) {
                    return Err(MacflyError::ScopeDenied(format!(
                        "Zielordner '{folder}' im Profil '{profile_name}' gesperrt"
                    )));
                }
            }
            Self::NotesMove {
                destination_folder,
                destination_account,
                ..
            } => {
                if let Some(acc) = destination_account
                    && !profile.notes.is_account_allowed(acc)
                {
                    return Err(MacflyError::ScopeDenied(format!(
                        "Ziel-Account '{acc}' im Profil '{profile_name}' nicht erlaubt"
                    )));
                }
                if !profile.notes.is_folder_allowed(destination_folder) {
                    return Err(MacflyError::ScopeDenied(format!(
                        "Zielordner '{destination_folder}' im Profil '{profile_name}' gesperrt"
                    )));
                }
            }
            Self::NotesMoveFolderContents {
                source_folder_path,
                destination_folder_path,
                destination_account,
                ..
            } => {
                if let Some(acc) = destination_account
                    && !profile.notes.is_account_allowed(acc)
                {
                    return Err(MacflyError::ScopeDenied(format!(
                        "Ziel-Account '{acc}' im Profil '{profile_name}' nicht erlaubt"
                    )));
                }
                if !profile.notes.is_folder_allowed(source_folder_path)
                    || !profile.notes.is_folder_allowed(destination_folder_path)
                {
                    return Err(MacflyError::ScopeDenied(format!(
                        "Verschiebung '{source_folder_path}' → '{destination_folder_path}' im Profil '{profile_name}' gesperrt"
                    )));
                }
            }
            Self::NotesCreateFolder { account, name, .. } => {
                if let Some(acc) = account
                    && !profile.notes.is_account_allowed(acc)
                {
                    return Err(MacflyError::ScopeDenied(format!(
                        "Notes-Account '{acc}' im Profil '{profile_name}' nicht erlaubt"
                    )));
                }
                if !profile.notes.is_folder_allowed(name) {
                    return Err(MacflyError::ScopeDenied(format!(
                        "Ordnername '{name}' im Profil '{profile_name}' gesperrt"
                    )));
                }
            }
            Self::NotesDeleteEmptyFolder { folder_name, .. } => {
                if !profile.notes.is_folder_allowed(folder_name) {
                    return Err(MacflyError::ScopeDenied(format!(
                        "Ordner '{folder_name}' im Profil '{profile_name}' gesperrt"
                    )));
                }
            }
            Self::NotesAppend { .. } => {}
            Self::MailCreateDraft { account, to, .. } => {
                if to.is_empty() {
                    return Err(MacflyError::PlanError(
                        "mail.create_draft benötigt mindestens einen Empfänger in `to`".to_string(),
                    ));
                }
                if let Some(acc) = account
                    && !profile.mail.is_account_allowed(acc)
                {
                    return Err(MacflyError::ScopeDenied(format!(
                        "Mail-Account '{acc}' im Profil '{profile_name}' nicht erlaubt"
                    )));
                }
            }
            Self::MailMarkRead {
                account, mailbox, ..
            } => {
                if !profile.mail.is_account_allowed(account)
                    || !profile.mail.is_mailbox_allowed(mailbox)
                {
                    return Err(MacflyError::ScopeDenied(format!(
                        "Postfach '{account}/{mailbox}' im Profil '{profile_name}' gesperrt"
                    )));
                }
            }
            Self::MailMove {
                account,
                from_mailbox,
                to_mailbox,
                ..
            } => {
                if !profile.mail.is_account_allowed(account)
                    || !profile.mail.is_mailbox_allowed(from_mailbox)
                    || !profile.mail.is_mailbox_allowed(to_mailbox)
                {
                    return Err(MacflyError::ScopeDenied(format!(
                        "Mail-Verschiebung in '{account}' im Profil '{profile_name}' gesperrt"
                    )));
                }
            }
            Self::ShortcutsRun { name, .. } => {
                if !profile.shortcuts.is_allowed(name) {
                    return Err(MacflyError::ScopeDenied(format!(
                        "Kurzbefehl '{name}' ist im Profil '{profile_name}' nicht in `shortcuts.allow` freigegeben"
                    )));
                }
            }
            Self::RemindersCreate { title, .. } => {
                if title.trim().is_empty() {
                    return Err(MacflyError::PlanError(
                        "reminders.create benötigt einen nicht-leeren Titel".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}

pub struct PlanStore {
    plans_dir: PathBuf,
}

impl PlanStore {
    pub fn default_store() -> Self {
        Self::new(data_dir().join("plans"))
    }

    pub fn new(plans_dir: PathBuf) -> Self {
        Self { plans_dir }
    }

    pub fn create(
        &self,
        profile_name: &str,
        profile: &ProfileConfig,
        proposal: PlanProposal,
        ttl_secs: u64,
    ) -> Result<StoredPlan, MacflyError> {
        if proposal.operations.is_empty() {
            return Err(MacflyError::PlanError(
                "Änderungsplan enthält keine Operationen".to_string(),
            ));
        }

        for op in &proposal.operations {
            op.validate_against_profile(profile_name, profile)?;
        }

        let now = now_unix();
        let canonical_ops = serde_json::to_string(&proposal.operations)?;
        let hash_input = format!("{profile_name}\n{canonical_ops}");
        let full_hash = compute_sha256_hex(&hash_input);
        let plan_hash = format!("sha256:{full_hash}");
        let approval_code = random_approval_code()?;
        let plan_id = format!("plan-{now}-{}", &full_hash[..8]);

        let mut risk_level = proposal
            .operations
            .iter()
            .map(PlanOperation::risk_level)
            .max()
            .unwrap_or(RiskLevel::Low);
        if proposal.operations.len() > 5 && risk_level < RiskLevel::High {
            risk_level = RiskLevel::High;
        }

        let reversibility = if proposal
            .operations
            .iter()
            .any(|o| o.reversibility() == Reversibility::Irreversible)
        {
            Reversibility::Irreversible
        } else if proposal
            .operations
            .iter()
            .any(|o| o.reversibility() == Reversibility::BestEffort)
        {
            Reversibility::BestEffort
        } else {
            Reversibility::Reversible
        };

        let preview_lines: Vec<String> = proposal
            .operations
            .iter()
            .map(PlanOperation::preview)
            .collect();
        let summary = proposal
            .summary
            .unwrap_or_else(|| format!("{} Operation(en)", proposal.operations.len()));

        let auto_approved = preset_auto_approves(profile.preset, risk_level, &proposal.operations);

        let plan = StoredPlan {
            schema_version: "1.0".to_string(),
            plan_id,
            profile: profile_name.to_string(),
            summary,
            created_at_unix: now,
            expires_at_unix: now + ttl_secs,
            status: if auto_approved {
                PlanStatus::Approved
            } else {
                PlanStatus::PendingApproval
            },
            risk_level,
            reversibility,
            plan_hash,
            approval_code,
            approved_at_unix: auto_approved.then_some(now),
            approved_by: auto_approved.then(|| format!("preset:{}", profile.preset.name())),
            applied_at_unix: None,
            operations: proposal.operations,
            preview_lines,
            completed_ops: Vec::new(),
        };

        self.save(&plan)?;
        Ok(plan)
    }

    pub fn load(&self, plan_id: &str) -> Result<StoredPlan, MacflyError> {
        let path = self.plan_path(plan_id)?;
        if !path.exists() {
            return Err(MacflyError::NotFound(format!(
                "Plan '{plan_id}' nicht gefunden"
            )));
        }
        let raw = fs::read_to_string(&path)?;
        let plan: StoredPlan = serde_json::from_str(&raw)?;
        verify_plan_integrity(&plan)?;
        Ok(plan)
    }

    pub fn list(&self) -> Result<Vec<StoredPlan>, MacflyError> {
        if !self.plans_dir.exists() {
            return Ok(Vec::new());
        }
        let mut plans = Vec::new();
        for entry in fs::read_dir(&self.plans_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(raw) = fs::read_to_string(&path)
                && let Ok(plan) = serde_json::from_str::<StoredPlan>(&raw)
            {
                plans.push(plan);
            }
        }
        plans.sort_by_key(|a| std::cmp::Reverse(a.created_at_unix));
        Ok(plans)
    }

    pub fn approve(&self, plan_id: &str, confirm_code: &str) -> Result<StoredPlan, MacflyError> {
        let mut plan = self.load(plan_id)?;
        let now = now_unix();
        if now > plan.expires_at_unix {
            return Err(MacflyError::ApprovalError(format!(
                "Plan '{plan_id}' ist abgelaufen (bitte neu erstellen)"
            )));
        }
        if plan.status != PlanStatus::PendingApproval {
            return Err(MacflyError::ApprovalError(format!(
                "Plan '{plan_id}' hat Status {:?} und kann nicht erneut freigegeben werden",
                plan.status
            )));
        }
        if !confirm_code
            .trim()
            .eq_ignore_ascii_case(&plan.approval_code)
        {
            return Err(MacflyError::ApprovalError(
                "Ungültiger Bestätigungscode".to_string(),
            ));
        }
        plan.status = PlanStatus::Approved;
        plan.approved_at_unix = Some(now);
        plan.approved_by = Some("human".to_string());
        self.save(&plan)?;
        Ok(plan)
    }

    pub fn save(&self, plan: &StoredPlan) -> Result<(), MacflyError> {
        ensure_private_dir(&self.plans_dir)?;
        let path = self.plan_path(&plan.plan_id)?;
        let json = serde_json::to_string_pretty(plan)?;
        write_private_file(&path, &format!("{json}\n"))?;
        Ok(())
    }

    pub fn acquire_lock(&self, plan_id: &str) -> Result<PlanLock, MacflyError> {
        ensure_private_dir(&self.plans_dir)?;
        let plan_path = self.plan_path(plan_id)?;
        let lock_path = plan_path.with_extension("lock");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    MacflyError::ApprovalError(format!(
                        "Plan '{plan_id}' wird bereits von einem anderen Prozess ausgeführt (Lock aktiv: {})",
                        lock_path.display()
                    ))
                } else {
                    MacflyError::Io(e)
                }
            })?;
        let _ = writeln!(file, "pid={} started_at={}", std::process::id(), now_unix());
        Ok(PlanLock { lock_path })
    }

    fn plan_path(&self, plan_id: &str) -> Result<PathBuf, MacflyError> {
        if plan_id.contains('/') || plan_id.contains('\\') || plan_id.contains("..") {
            return Err(MacflyError::PlanError(format!(
                "Ungültige Plan-ID '{plan_id}'"
            )));
        }
        Ok(self.plans_dir.join(format!("{plan_id}.json")))
    }
}

pub fn verify_plan_integrity(plan: &StoredPlan) -> Result<(), MacflyError> {
    let canonical_ops = serde_json::to_string(&plan.operations)?;
    let expected = format!(
        "sha256:{}",
        compute_sha256_hex(&format!("{}\n{}", plan.profile, canonical_ops))
    );
    if plan.plan_hash != expected {
        return Err(MacflyError::ApprovalError(format!(
            "Integritätsfehler: Plan '{}' wurde nach Erstellung verändert!",
            plan.plan_id
        )));
    }
    Ok(())
}

/// 6-stelliger Prüfcode aus `/dev/urandom` — nicht aus dem Plan-Hash ableitbar.
fn random_approval_code() -> Result<String, MacflyError> {
    let mut buf = [0u8; 3];
    fs::File::open("/dev/urandom")?.read_exact(&mut buf)?;
    Ok(buf.iter().map(|b| format!("{b:02X}")).collect())
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn ensure_private_dir(path: &Path) -> Result<(), MacflyError> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

pub fn write_private_file(path: &Path, content: &str) -> Result<(), MacflyError> {
    fs::write(path, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn default_schema_version() -> String {
    "1.0".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(tag: &str) -> PlanStore {
        let dir = std::env::temp_dir().join(format!("macfly-test-{tag}-{}", now_unix()));
        let _ = fs::remove_dir_all(&dir);
        PlanStore::new(dir)
    }

    #[test]
    fn plan_create_approve_and_tamper_detection() {
        let store = temp_store("tamper");
        let profile = ProfileConfig::default();
        let proposal = PlanProposal {
            schema_version: "1.0".to_string(),
            summary: Some("Ordner aufräumen".to_string()),
            operations: vec![PlanOperation::NotesMove {
                note_id: "x-coredata://123".to_string(),
                expected_revision: None,
                destination_folder: "Archiv".to_string(),
                destination_account: None,
            }],
        };

        let plan = store.create("default", &profile, proposal, 600).unwrap();
        assert_eq!(plan.status, PlanStatus::PendingApproval);
        assert_eq!(plan.risk_level, RiskLevel::Medium);
        assert_eq!(plan.approval_code.len(), 6);
        assert!(
            !plan
                .plan_hash
                .to_ascii_uppercase()
                .contains(&plan.approval_code)
        );
        assert!(
            !serde_json::to_string(&plan.for_output())
                .unwrap()
                .contains("approval_code")
        );

        // Falscher Code wird abgewiesen
        assert!(store.approve(&plan.plan_id, "000000").is_err());

        // Richtiger Code gibt frei
        let wrong = store
            .approve(&plan.plan_id, "000000")
            .unwrap_err()
            .to_string();
        assert!(!wrong.contains(&plan.approval_code));
        let approved = store.approve(&plan.plan_id, &plan.approval_code).unwrap();
        assert_eq!(approved.status, PlanStatus::Approved);
        assert_eq!(approved.approved_by.as_deref(), Some("human"));

        // Manipulation der gespeicherten Datei wird erkannt
        let path = store.plan_path(&plan.plan_id).unwrap();
        let raw = fs::read_to_string(&path)
            .unwrap()
            .replace("Archiv", "Manipuliert");
        fs::write(&path, raw).unwrap();
        assert!(store.load(&plan.plan_id).is_err());
    }

    fn create_folder_op(name: &str) -> PlanOperation {
        PlanOperation::NotesCreateFolder {
            account: None,
            name: name.to_string(),
            parent_folder: None,
        }
    }

    fn move_op() -> PlanOperation {
        PlanOperation::NotesMove {
            note_id: "x-coredata://1".to_string(),
            expected_revision: None,
            destination_folder: "Archiv".to_string(),
            destination_account: None,
        }
    }

    fn create_with(preset: Preset, ops: Vec<PlanOperation>) -> StoredPlan {
        let store = temp_store(&format!("preset-{}-{}", preset.name(), ops.len()));
        let profile = ProfileConfig {
            preset,
            ..ProfileConfig::default()
        };
        let proposal = PlanProposal {
            schema_version: "1.0".to_string(),
            summary: None,
            operations: ops,
        };
        store.create("default", &profile, proposal, 600).unwrap()
    }

    #[test]
    fn presets_decide_auto_approval() {
        // strict: nichts automatisch
        let p = create_with(Preset::Strict, vec![create_folder_op("A")]);
        assert_eq!(p.status, PlanStatus::PendingApproval);
        assert_eq!(p.approved_by, None);

        // balanced: risikoarme Anlege-Aktion automatisch …
        let p = create_with(Preset::Balanced, vec![create_folder_op("A")]);
        assert_eq!(p.status, PlanStatus::Approved);
        assert_eq!(p.approved_by.as_deref(), Some("preset:balanced"));
        // … Verschieben nicht …
        let p = create_with(Preset::Balanced, vec![move_op()]);
        assert_eq!(p.status, PlanStatus::PendingApproval);
        // … und mehr als 5 Operationen auch nicht
        let many = (0..6).map(|i| create_folder_op(&format!("F{i}"))).collect();
        let p = create_with(Preset::Balanced, many);
        assert_eq!(p.status, PlanStatus::PendingApproval);

        // open: Verschieben automatisch, auch in großen Mengen
        let many_moves = (0..8).map(|_| move_op()).collect();
        let p = create_with(Preset::Open, many_moves);
        assert_eq!(p.status, PlanStatus::Approved);
        assert_eq!(p.approved_by.as_deref(), Some("preset:open"));
    }

    #[test]
    fn open_preset_still_requires_approval_for_shortcuts() {
        let store = temp_store("open-shortcuts");
        let mut profile = ProfileConfig {
            preset: Preset::Open,
            ..ProfileConfig::default()
        };
        profile.shortcuts.allow.push("Office Focus".to_string());
        let proposal = PlanProposal {
            schema_version: "1.0".to_string(),
            summary: None,
            operations: vec![PlanOperation::ShortcutsRun {
                name: "Office Focus".to_string(),
                input: None,
            }],
        };
        let p = store.create("default", &profile, proposal, 600).unwrap();
        assert_eq!(p.status, PlanStatus::PendingApproval);
    }

    #[test]
    fn action_risk_catalog_matches_operations() {
        let samples = vec![
            create_folder_op("A"),
            move_op(),
            PlanOperation::ShortcutsRun {
                name: "X".to_string(),
                input: None,
            },
        ];
        for op in samples {
            let action = serde_json::to_value(&op).unwrap()["action"]
                .as_str()
                .unwrap()
                .to_string();
            let (_, risk) = ACTION_RISKS
                .iter()
                .find(|(name, _)| *name == action)
                .expect("action in catalog");
            assert_eq!(*risk, op.risk_level());
        }
    }

    #[test]
    fn blocks_denied_folder_in_plan() {
        let store = temp_store("scope");
        let profile = ProfileConfig::default();
        let proposal = PlanProposal {
            schema_version: "1.0".to_string(),
            summary: None,
            operations: vec![PlanOperation::NotesMove {
                note_id: "x-coredata://123".to_string(),
                expected_revision: None,
                destination_folder: "Passwörter".to_string(),
                destination_account: None,
            }],
        };
        let err = store
            .create("default", &profile, proposal, 600)
            .unwrap_err();
        assert_eq!(err.code(), "SCOPE_DENIED");
    }

    #[test]
    fn shortcuts_run_requires_explicit_allowlist() {
        let store = temp_store("shortcuts");
        let mut profile = ProfileConfig::default();
        let proposal = PlanProposal {
            schema_version: "1.0".to_string(),
            summary: Some("Fokus-Szene".to_string()),
            operations: vec![PlanOperation::ShortcutsRun {
                name: "Office Focus".to_string(),
                input: None,
            }],
        };

        let err = store
            .create("default", &profile, proposal.clone(), 600)
            .unwrap_err();
        assert_eq!(err.code(), "SCOPE_DENIED");

        profile.shortcuts.allow.push("Office Focus".to_string());
        let ok = store.create("default", &profile, proposal, 600).unwrap();
        assert_eq!(ok.risk_level, RiskLevel::High);
        assert_eq!(ok.reversibility, Reversibility::Irreversible);
    }

    #[test]
    fn plan_lock_prevents_concurrent_execution() {
        let store = temp_store("lock");
        let lock1 = store.acquire_lock("plan-123").unwrap();
        let err = store.acquire_lock("plan-123").err().unwrap();
        assert_eq!(err.code(), "APPROVAL_ERROR");
        drop(lock1);
        assert!(store.acquire_lock("plan-123").is_ok());
    }

    #[test]
    fn detailed_preview_shows_full_mail_draft_body_and_notes_diff() {
        let draft = PlanOperation::MailCreateDraft {
            account: Some("Exchange".to_string()),
            to: vec!["billing@acme.example".to_string()],
            cc: vec![],
            subject: "Re: Invoice #42".to_string(),
            body: "Line one\nLine two".to_string(),
            signature_name: Some("Corporate".to_string()),
            reply_to_local_id: None,
        };
        let lines = draft.detailed_preview().join("\n");
        assert!(lines.contains("| Line one"));
        assert!(lines.contains("| Line two"));
        assert!(lines.contains("Signature: Corporate"));

        let note = PlanOperation::NotesAppend {
            note_id: "x-coredata://note/1".to_string(),
            expected_revision: Some("sha256:abc".to_string()),
            text: "New bullet".to_string(),
        };
        let note_lines = note.detailed_preview().join("\n");
        assert!(note_lines.contains("+ New bullet"));
    }
}
