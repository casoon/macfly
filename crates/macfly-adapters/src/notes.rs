use std::collections::BTreeMap;

use macfly_core::config::{DlpConfig, ProfileConfig};
use macfly_core::dlp::apply_dlp;
use macfly_core::error::MacflyError;
use macfly_core::html::{analyze_note_fidelity, html_to_clean_text};
use macfly_core::model::{
    AccountSummary, DuplicateTitleGroup, FolderSummary, NoteDetail, NoteSummary, NotesAuditReport,
    NotesOverview, TrustLevel, compute_revision_hash,
};
use macfly_core::plan::PlanOperation;
use serde::{Deserialize, Serialize};

use crate::jxa::run_jxa;

const SCRIPT_OVERVIEW: &str = include_str!("../scripts/notes_overview.js");
const SCRIPT_LIST: &str = include_str!("../scripts/notes_list.js");
const SCRIPT_READ: &str = include_str!("../scripts/notes_read.js");
const SCRIPT_MUTATE: &str = include_str!("../scripts/notes_mutate.js");

pub struct NotesAdapter {
    timeout_secs: u64,
}

#[derive(Debug, Default)]
pub struct ListNotesQuery {
    pub account: Option<String>,
    pub folder: Option<String>,
    pub query: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Serialize)]
pub struct NotesListResult {
    pub schema_version: String,
    pub profile: String,
    pub total_matched: usize,
    pub returned: usize,
    pub notes: Vec<NoteSummary>,
}

#[derive(Debug, Deserialize)]
struct RawOverview {
    accounts: Vec<AccountSummary>,
    folders: Vec<RawFolder>,
}

#[derive(Debug, Deserialize)]
struct RawFolder {
    id: String,
    name: String,
    #[serde(default)]
    path: String,
    account: String,
    note_count: usize,
    #[serde(default)]
    subfolder_count: usize,
}

#[derive(Debug, Serialize)]
struct ListPayload<'a> {
    account: Option<&'a str>,
    folder: Option<&'a str>,
    query: Option<&'a str>,
    allow_accounts: &'a [String],
    allow_folders: &'a [String],
    deny_folders: &'a [String],
    skip_password_protected: bool,
    limit: usize,
}

#[derive(Debug, Deserialize)]
struct RawListResponse {
    total_matched: usize,
    notes: Vec<NoteSummary>,
}

#[derive(Debug, Serialize)]
struct ReadPayload<'a> {
    id: &'a str,
}

#[derive(Debug, Deserialize)]
struct RawReadResponse {
    found: bool,
    id: Option<String>,
    title: Option<String>,
    account: Option<String>,
    folder: Option<String>,
    created_at: Option<String>,
    modified_at: Option<String>,
    password_protected: Option<bool>,
    attachment_count: Option<usize>,
    body_html: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawMutateResponse {
    ok: bool,
    detail: String,
}

impl NotesAdapter {
    pub fn new(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }

    pub fn overview(
        &self,
        profile_name: &str,
        profile: &ProfileConfig,
    ) -> Result<NotesOverview, MacflyError> {
        let raw: RawOverview = run_jxa(
            "Apple Notes",
            SCRIPT_OVERVIEW,
            &serde_json::json!({}),
            self.timeout_secs,
        )?;

        let mut hidden_folders = 0usize;
        let folders: Vec<FolderSummary> = raw
            .folders
            .into_iter()
            .map(|f| {
                let path = if f.path.is_empty() {
                    f.name.clone()
                } else {
                    f.path
                };
                let allowed = profile.notes.is_account_allowed(&f.account)
                    && profile.notes.is_folder_allowed(&f.name)
                    && profile.notes.is_folder_allowed(&path);
                if !allowed {
                    hidden_folders += 1;
                }
                FolderSummary {
                    id: f.id,
                    name: f.name,
                    path,
                    account: f.account,
                    note_count: f.note_count,
                    subfolder_count: f.subfolder_count,
                    allowed_by_policy: allowed,
                }
            })
            .collect();

        let total_notes = folders.iter().map(|f| f.note_count).sum();
        let total_folders = folders.len();
        let total_accounts = raw.accounts.len();

        Ok(NotesOverview {
            schema_version: "1.0".to_string(),
            profile: profile_name.to_string(),
            total_accounts,
            total_folders,
            total_notes,
            hidden_by_policy_folders: hidden_folders,
            accounts: raw.accounts,
            folders,
        })
    }

    pub fn list(
        &self,
        profile_name: &str,
        profile: &ProfileConfig,
        q: &ListNotesQuery,
    ) -> Result<NotesListResult, MacflyError> {
        if let Some(folder) = &q.folder
            && !profile.notes.is_folder_allowed(folder)
        {
            return Err(MacflyError::ScopeDenied(format!(
                "Ordner '{folder}' ist im Profil '{profile_name}' gesperrt"
            )));
        }
        if let Some(account) = &q.account
            && !profile.notes.is_account_allowed(account)
        {
            return Err(MacflyError::ScopeDenied(format!(
                "Account '{account}' ist im Profil '{profile_name}' nicht freigegeben"
            )));
        }

        let payload = ListPayload {
            account: q.account.as_deref(),
            folder: q.folder.as_deref(),
            query: q.query.as_deref(),
            allow_accounts: &profile.notes.allow_accounts,
            allow_folders: &profile.notes.allow_folders,
            deny_folders: &profile.notes.deny_folders,
            skip_password_protected: profile.notes.skip_password_protected,
            limit: q.limit,
        };

        let raw: RawListResponse =
            run_jxa("Apple Notes", SCRIPT_LIST, &payload, self.timeout_secs)?;
        let returned = raw.notes.len();

        Ok(NotesListResult {
            schema_version: "1.0".to_string(),
            profile: profile_name.to_string(),
            total_matched: raw.total_matched,
            returned,
            notes: raw.notes,
        })
    }

    pub fn audit(
        &self,
        profile_name: &str,
        profile: &ProfileConfig,
    ) -> Result<NotesAuditReport, MacflyError> {
        let ov = self.overview(profile_name, profile)?;
        let allowed_folders: Vec<FolderSummary> = ov
            .folders
            .into_iter()
            .filter(|f| f.allowed_by_policy)
            .collect();

        let empty_folders: Vec<FolderSummary> = allowed_folders
            .iter()
            .filter(|f| f.note_count == 0 && f.subfolder_count == 0)
            .cloned()
            .collect();

        let single_note_folders: Vec<FolderSummary> = allowed_folders
            .iter()
            .filter(|f| f.note_count == 1)
            .cloned()
            .collect();

        let mut by_folder_name: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for f in &allowed_folders {
            by_folder_name
                .entry(f.name.clone())
                .or_default()
                .push(f.path.clone());
        }
        let duplicate_folder_names: BTreeMap<String, Vec<String>> = by_folder_name
            .into_iter()
            .filter(|(_, paths)| paths.len() > 1)
            .collect();

        let all_notes = self.list(
            profile_name,
            profile,
            &ListNotesQuery {
                limit: 0,
                ..Default::default()
            },
        )?;

        let mut by_title: BTreeMap<String, Vec<NoteSummary>> = BTreeMap::new();
        for note in &all_notes.notes {
            let norm = note.title.trim().to_lowercase();
            if norm.is_empty() || norm == "(ohne titel)" {
                continue;
            }
            by_title.entry(norm).or_default().push(note.clone());
        }

        let mut duplicate_titles: Vec<DuplicateTitleGroup> = by_title
            .into_iter()
            .filter(|(_, group)| group.len() > 1)
            .map(|(normalized_title, notes)| DuplicateTitleGroup {
                normalized_title,
                count: notes.len(),
                notes,
            })
            .collect();
        duplicate_titles.sort_by_key(|a| std::cmp::Reverse(a.count));

        let mut oldest = all_notes.notes.clone();
        oldest.sort_by(|a, b| {
            let am = a.modified_at.as_deref().unwrap_or("9999");
            let bm = b.modified_at.as_deref().unwrap_or("9999");
            am.cmp(bm)
        });
        oldest.truncate(20);

        Ok(NotesAuditReport {
            schema_version: "1.0".to_string(),
            profile: profile_name.to_string(),
            total_folders_scanned: allowed_folders.len(),
            total_notes_scanned: all_notes.notes.len(),
            empty_folders,
            single_note_folders,
            duplicate_folder_names,
            duplicate_titles,
            oldest_unmodified_notes: oldest,
        })
    }

    pub fn read(
        &self,
        profile_name: &str,
        profile: &ProfileConfig,
        id: &str,
        include_raw_html: bool,
        disable_redaction: bool,
    ) -> Result<NoteDetail, MacflyError> {
        let raw: RawReadResponse = run_jxa(
            "Apple Notes",
            SCRIPT_READ,
            &ReadPayload { id },
            self.timeout_secs,
        )?;

        if !raw.found {
            return Err(MacflyError::NotFound(format!(
                "Notiz '{id}' nicht gefunden"
            )));
        }

        let account = raw.account.unwrap_or_default();
        let folder = raw.folder.unwrap_or_default();
        let is_protected = raw.password_protected.unwrap_or(false);

        if !profile.notes.is_account_allowed(&account) {
            return Err(MacflyError::ScopeDenied(format!(
                "Account '{account}' ist im Profil '{profile_name}' nicht freigegeben"
            )));
        }
        if !profile.notes.is_folder_allowed(&folder) {
            return Err(MacflyError::ScopeDenied(format!(
                "Ordner '{folder}' ist im Profil '{profile_name}' gesperrt"
            )));
        }
        if profile.notes.skip_password_protected && is_protected {
            return Err(MacflyError::ScopeDenied(
                "Passwortgeschützte Notiz ist laut Profil gesperrt".to_string(),
            ));
        }

        let html = raw.body_html.unwrap_or_default();
        let attachment_count = raw.attachment_count.unwrap_or(0);
        let title = raw.title.unwrap_or_else(|| "(Ohne Titel)".to_string());
        let revision = compute_revision_hash(&format!("{title}\n{html}"));
        let fidelity = analyze_note_fidelity(&html, attachment_count);
        let clean_text = html_to_clean_text(&html);

        let dlp_cfg = if disable_redaction {
            DlpConfig {
                enabled: false,
                ..profile.dlp.clone()
            }
        } else {
            profile.dlp.clone()
        };
        let redacted = apply_dlp(&clean_text, &dlp_cfg);

        Ok(NoteDetail {
            schema_version: "1.0".to_string(),
            id: raw.id.unwrap_or_else(|| id.to_string()),
            title,
            account,
            folder_path: folder.clone(),
            folder,
            created_at: raw.created_at,
            modified_at: raw.modified_at,
            revision,
            trust_level: TrustLevel::UserAuthored,
            password_protected: is_protected,
            attachment_count,
            fidelity,
            redactions: redacted.counts,
            truncated: redacted.truncated,
            body_text: redacted.text,
            body_html: if include_raw_html { Some(html) } else { None },
        })
    }

    pub fn execute_operation(
        &self,
        profile_name: &str,
        profile: &ProfileConfig,
        op: &PlanOperation,
    ) -> Result<String, MacflyError> {
        // Pre-Flight Revision Check bei Append/Move falls angegeben
        match op {
            PlanOperation::NotesAppend {
                note_id,
                expected_revision: Some(exp),
                ..
            }
            | PlanOperation::NotesMove {
                note_id,
                expected_revision: Some(exp),
                ..
            } => {
                let current = self.read(profile_name, profile, note_id, false, true)?;
                if current.revision != *exp {
                    return Err(MacflyError::PlanError(format!(
                        "Revision-Konflikt bei Notiz '{}': erwartet {}, aktuell {}",
                        note_id, exp, current.revision
                    )));
                }
            }
            _ => {}
        }

        let payload = match op {
            PlanOperation::NotesCreate {
                account,
                folder,
                title,
                body,
            } => serde_json::json!({
                "action": "notes_create",
                "account": account,
                "folder": folder,
                "title": title,
                "body": body,
            }),
            PlanOperation::NotesAppend { note_id, text, .. } => serde_json::json!({
                "action": "notes_append",
                "note_id": note_id,
                "text": text,
            }),
            PlanOperation::NotesMove {
                note_id,
                destination_folder,
                destination_account,
                ..
            } => serde_json::json!({
                "action": "notes_move",
                "note_id": note_id,
                "destination_folder": destination_folder,
                "destination_account": destination_account,
            }),
            PlanOperation::NotesMoveFolderContents {
                source_folder_id,
                source_folder_path,
                destination_folder_path,
                destination_account,
            } => serde_json::json!({
                "action": "notes_move_folder_contents",
                "source_folder_id": source_folder_id,
                "source_folder_path": source_folder_path,
                "destination_folder_path": destination_folder_path,
                "destination_account": destination_account,
            }),
            PlanOperation::NotesCreateFolder {
                account,
                name,
                parent_folder,
            } => serde_json::json!({
                "action": "notes_create_folder",
                "account": account,
                "name": name,
                "parent_folder": parent_folder,
            }),
            PlanOperation::NotesDeleteEmptyFolder {
                folder_id,
                folder_name,
            } => serde_json::json!({
                "action": "notes_delete_empty_folder",
                "folder_id": folder_id,
                "folder_name": folder_name,
            }),
            _ => {
                return Err(MacflyError::PlanError(
                    "Operation gehört nicht zum Notes-Adapter".to_string(),
                ));
            }
        };

        let res: RawMutateResponse =
            run_jxa("Apple Notes", SCRIPT_MUTATE, &payload, self.timeout_secs)?;
        if !res.ok {
            return Err(MacflyError::ScriptError(res.detail));
        }
        Ok(res.detail)
    }
}
