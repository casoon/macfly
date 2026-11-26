use std::collections::BTreeMap;

use macfly_core::error::MacflyError;
use macfly_core::model::{ContactSummary, ContactsAuditReport, ContactsListResult};
use serde::Deserialize;

use crate::jxa::run_jxa;

const SCRIPT_LIST: &str = include_str!("../scripts/contacts_list.js");

pub struct ContactsAdapter {
    timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
struct RawContactsResponse {
    total_matched: usize,
    contacts: Vec<ContactSummary>,
}

impl ContactsAdapter {
    pub fn new(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }

    pub fn list(
        &self,
        query: Option<&str>,
        limit: usize,
    ) -> Result<ContactsListResult, MacflyError> {
        let payload = serde_json::json!({
            "query": query,
            "limit": limit,
        });
        let raw: RawContactsResponse =
            run_jxa("Apple Contacts", SCRIPT_LIST, &payload, self.timeout_secs)?;
        let returned = raw.contacts.len();
        Ok(ContactsListResult {
            schema_version: "1.0".to_string(),
            total_matched: raw.total_matched,
            returned,
            contacts: raw.contacts,
        })
    }

    pub fn audit(&self) -> Result<ContactsAuditReport, MacflyError> {
        let all = self.list(None, 0)?;
        let mut by_name: BTreeMap<String, usize> = BTreeMap::new();
        let mut by_email: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut empty_contacts = Vec::new();

        for c in &all.contacts {
            let norm_name = c.name.trim().to_lowercase();
            if !norm_name.is_empty() && norm_name != "(unbenannt)" {
                *by_name.entry(c.name.clone()).or_insert(0) += 1;
            }
            for email in &c.emails {
                let norm_email = email.trim().to_lowercase();
                if !norm_email.is_empty() {
                    by_email.entry(norm_email).or_default().push(c.name.clone());
                }
            }
            if c.emails.is_empty() && c.phones.is_empty() {
                empty_contacts.push(c.clone());
            }
        }

        let duplicate_names: BTreeMap<String, usize> = by_name
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .collect();
        let duplicate_emails: BTreeMap<String, Vec<String>> = by_email
            .into_iter()
            .filter(|(_, names)| names.len() > 1)
            .collect();

        Ok(ContactsAuditReport {
            schema_version: "1.0".to_string(),
            total_contacts: all.contacts.len(),
            duplicate_names,
            duplicate_emails,
            empty_contacts,
        })
    }
}
