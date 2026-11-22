use macfly_core::error::MacflyError;
use macfly_core::model::{SafariTab, SafariTabsResult};
use serde::Deserialize;

use crate::jxa::run_jxa;

const SCRIPT_TABS: &str = include_str!("../scripts/safari_tabs.js");

pub struct SafariAdapter {
    timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
struct RawSafariTabs {
    total_tabs: usize,
    tabs: Vec<SafariTab>,
}

impl SafariAdapter {
    pub fn new(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }

    pub fn tabs(&self, query: Option<&str>) -> Result<SafariTabsResult, MacflyError> {
        let payload = serde_json::json!({ "query": query });
        let raw: RawSafariTabs =
            run_jxa("Apple Safari", SCRIPT_TABS, &payload, self.timeout_secs)?;
        Ok(SafariTabsResult {
            schema_version: "1.0".to_string(),
            total_tabs: raw.total_tabs,
            tabs: raw.tabs,
        })
    }
}
