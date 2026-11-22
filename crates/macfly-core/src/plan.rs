use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config::{ProfileConfig, data_dir};
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
    Applied,
    Failed,
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
    pub approval_code: String,
    #[serde(default)]
    pub approved_at_unix: Option<u64>,
    #[serde(default)]
    pub applied_at_unix: Option<u64>,
    pub operations: Vec<PlanOperation>,
    pub preview_lines: Vec<String>,
}

impl PlanOperation {
    pub fn risk_level(&self) -> RiskLevel {
        match self {
            Self::NotesCreate { .. }
            | Self::NotesCreateFolder { .. }
            | Self::MailCreateDraft { .. }
            | Self::MailMarkRead { .. } => RiskLevel::Low,
            Self::NotesAppend { .. }
            | Self::NotesMove { .. }
            | Self::NotesMoveFolderContents { .. }
            | Self::NotesDeleteEmptyFolder { .. }
            | Self::MailMove { .. } => RiskLevel::Medium,
        }
    }

    pub fn reversibility(&self) -> Reversibility {
        match self {
            Self::NotesAppend { .. } => Reversibility::BestEffort,
            _ => Reversibility::Reversible,
        }
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
        let approval_code = full_hash[..6].to_ascii_uppercase();
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

        let preview_lines: Vec<String> =
            proposal.operations.iter().map(PlanOperation::preview).collect();
        let summary = proposal
            .summary
            .unwrap_or_else(|| format!("{} Operation(en)", proposal.operations.len()));

        let plan = StoredPlan {
            schema_version: "1.0".to_string(),
            plan_id,
            profile: profile_name.to_string(),
            summary,
            created_at_unix: now,
            expires_at_unix: now + ttl_secs,
            status: PlanStatus::PendingApproval,
            risk_level,
            reversibility,
            plan_hash,
            approval_code,
            approved_at_unix: None,
            applied_at_unix: None,
            operations: proposal.operations,
            preview_lines,
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
        plans.sort_by(|a, b| b.created_at_unix.cmp(&a.created_at_unix));
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
        if !confirm_code.trim().eq_ignore_ascii_case(&plan.approval_code) {
            return Err(MacflyError::ApprovalError(format!(
                "Ungültiger Bestätigungscode (erwartet: {})",
                plan.approval_code
            )));
        }
        plan.status = PlanStatus::Approved;
        plan.approved_at_unix = Some(now);
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

        // Falscher Code wird abgewiesen
        assert!(store.approve(&plan.plan_id, "000000").is_err());

        // Richtiger Code gibt frei
        let approved = store.approve(&plan.plan_id, &plan.approval_code).unwrap();
        assert_eq!(approved.status, PlanStatus::Approved);

        // Manipulation der gespeicherten Datei wird erkannt
        let path = store.plan_path(&plan.plan_id).unwrap();
        let raw = fs::read_to_string(&path).unwrap().replace("Archiv", "Manipuliert");
        fs::write(&path, raw).unwrap();
        assert!(store.load(&plan.plan_id).is_err());
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
        let err = store.create("default", &profile, proposal, 600).unwrap_err();
        assert_eq!(err.code(), "SCOPE_DENIED");
    }
}
