use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    UserAuthored,
    UntrustedExternal,
}

// ---------- Apple Notes ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSummary {
    pub id: String,
    pub name: String,
    pub folder_count: usize,
    pub note_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderSummary {
    pub id: String,
    pub name: String,
    pub path: String,
    pub account: String,
    pub note_count: usize,
    #[serde(default)]
    pub subfolder_count: usize,
    pub allowed_by_policy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotesOverview {
    pub schema_version: String,
    pub profile: String,
    pub total_accounts: usize,
    pub total_folders: usize,
    pub total_notes: usize,
    pub hidden_by_policy_folders: usize,
    pub accounts: Vec<AccountSummary>,
    pub folders: Vec<FolderSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteSummary {
    pub id: String,
    pub title: String,
    pub account: String,
    pub folder: String,
    #[serde(default)]
    pub folder_path: String,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub password_protected: bool,
    pub attachment_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FidelityInfo {
    pub has_attachments: bool,
    pub has_tables: bool,
    pub has_checklists: bool,
    pub has_internal_links: bool,
    pub has_images: bool,
    pub safe_mutations: Vec<String>,
    pub blocked_mutations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteDetail {
    pub schema_version: String,
    pub id: String,
    pub title: String,
    pub account: String,
    pub folder: String,
    pub folder_path: String,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub revision: String,
    pub trust_level: TrustLevel,
    pub password_protected: bool,
    pub attachment_count: usize,
    pub fidelity: FidelityInfo,
    pub redactions: BTreeMap<String, usize>,
    pub truncated: bool,
    pub body_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_html: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateTitleGroup {
    pub normalized_title: String,
    pub count: usize,
    pub notes: Vec<NoteSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotesAuditReport {
    pub schema_version: String,
    pub profile: String,
    pub total_folders_scanned: usize,
    pub total_notes_scanned: usize,
    pub empty_folders: Vec<FolderSummary>,
    pub single_note_folders: Vec<FolderSummary>,
    pub duplicate_folder_names: BTreeMap<String, Vec<String>>,
    pub duplicate_titles: Vec<DuplicateTitleGroup>,
    pub oldest_unmodified_notes: Vec<NoteSummary>,
}

// ---------- Apple Mail ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailAccountSummary {
    pub id: String,
    pub name: String,
    pub email_addresses: Vec<String>,
    pub mailbox_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailboxSummary {
    pub name: String,
    pub account: String,
    pub unread_count: usize,
    pub message_count: usize,
    pub allowed_by_policy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailSignature {
    pub name: String,
    pub content_preview: String,
    pub char_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailOverview {
    pub schema_version: String,
    pub profile: String,
    pub total_accounts: usize,
    pub total_mailboxes: usize,
    pub total_unread: usize,
    pub accounts: Vec<MailAccountSummary>,
    pub mailboxes: Vec<MailboxSummary>,
    pub signatures: Vec<MailSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailMessageSummary {
    /// Stabile ID: `mail:<account>:<rfc_hash_or_local_id>`
    pub id: String,
    pub local_id: String,
    pub rfc_message_id: Option<String>,
    pub account: String,
    pub mailbox: String,
    pub subject: String,
    pub sender: String,
    pub date_received: Option<String>,
    pub read: bool,
    pub flagged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailMessageDetail {
    pub schema_version: String,
    pub id: String,
    pub local_id: String,
    pub rfc_message_id: Option<String>,
    pub account: String,
    pub mailbox: String,
    pub subject: String,
    pub sender: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub date_received: Option<String>,
    pub read: bool,
    pub flagged: bool,
    pub revision: String,
    pub trust_level: TrustLevel,
    pub content_warning: String,
    pub redactions: BTreeMap<String, usize>,
    pub truncated: bool,
    pub body_text: String,
}

// ---------- Apple Calendar & Reminders ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub calendar: String,
    pub title: String,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub all_day: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReminderItem {
    pub list: String,
    pub title: String,
    pub due_at: Option<String>,
    pub overdue: bool,
    pub priority: u8,
}

// ---------- Apple Shortcuts ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutSummary {
    pub name: String,
    pub allowed_by_policy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutsListResult {
    pub schema_version: String,
    pub profile: String,
    pub total: usize,
    pub allowed_count: usize,
    pub shortcuts: Vec<ShortcutSummary>,
}

// ---------- Apple Contacts ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactSummary {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    pub emails: Vec<String>,
    pub phones: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactsListResult {
    pub schema_version: String,
    pub total_matched: usize,
    pub returned: usize,
    pub contacts: Vec<ContactSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactsAuditReport {
    pub schema_version: String,
    pub total_contacts: usize,
    pub duplicate_names: BTreeMap<String, usize>,
    pub duplicate_emails: BTreeMap<String, Vec<String>>,
    pub empty_contacts: Vec<ContactSummary>,
}

// ---------- Apple Safari ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafariTab {
    pub window_index: usize,
    pub tab_index: usize,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafariTabsResult {
    pub schema_version: String,
    pub total_tabs: usize,
    pub tabs: Vec<SafariTab>,
}

// ---------- Context Packs ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyBrief {
    pub schema_version: String,
    pub profile: String,
    pub generated_at_unix: u64,
    pub upcoming_events: Vec<CalendarEvent>,
    pub open_reminders: Vec<ReminderItem>,
    pub unread_mail_count: usize,
    pub unread_mails: Vec<MailMessageSummary>,
    pub recent_notes: Vec<NoteSummary>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonContext {
    pub schema_version: String,
    pub profile: String,
    pub query: String,
    pub generated_at_unix: u64,
    pub contacts: Vec<ContactSummary>,
    pub recent_mails: Vec<MailMessageSummary>,
    pub matching_notes: Vec<NoteSummary>,
    pub upcoming_events: Vec<CalendarEvent>,
    pub open_reminders: Vec<ReminderItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingPrepContext {
    pub schema_version: String,
    pub profile: String,
    pub query: String,
    pub generated_at_unix: u64,
    pub target_event: Option<CalendarEvent>,
    pub related_contacts: Vec<ContactSummary>,
    pub related_mails: Vec<MailMessageSummary>,
    pub related_notes: Vec<NoteSummary>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyReviewContext {
    pub schema_version: String,
    pub profile: String,
    pub generated_at_unix: u64,
    pub recent_notes: Vec<NoteSummary>,
    pub unread_mails: Vec<MailMessageSummary>,
    pub overdue_reminders: Vec<ReminderItem>,
    pub upcoming_events: Vec<CalendarEvent>,
    pub open_safari_tabs: Vec<SafariTab>,
    pub warnings: Vec<String>,
}

pub fn compute_revision_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{}", &hex[..16])
}

pub fn compute_sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}
