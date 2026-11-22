use macfly_core::error::MacflyError;
use macfly_core::model::{CalendarEvent, ReminderItem};
use macfly_core::plan::PlanOperation;
use serde::{Deserialize, Serialize};

use crate::jxa::run_jxa;

const SCRIPT_CALENDAR: &str = include_str!("../scripts/calendar_events.js");
const SCRIPT_REMINDERS: &str = include_str!("../scripts/reminders_list.js");
const SCRIPT_REMINDERS_MUTATE: &str = include_str!("../scripts/reminders_mutate.js");

#[derive(Debug, Deserialize)]
struct RawMutateResponse {
    ok: bool,
    detail: String,
}

pub struct CalendarAdapter {
    timeout_secs: u64,
}

pub struct RemindersAdapter {
    timeout_secs: u64,
}

#[derive(Debug, Serialize)]
pub struct CalendarEventsResult {
    pub schema_version: String,
    pub days: u32,
    pub total_matched: usize,
    pub returned: usize,
    pub events: Vec<CalendarEvent>,
}

#[derive(Debug, Serialize)]
pub struct RemindersListResult {
    pub schema_version: String,
    pub total_matched: usize,
    pub returned: usize,
    pub reminders: Vec<ReminderItem>,
}

#[derive(Debug, Deserialize)]
struct RawCalendarResponse {
    total_matched: usize,
    events: Vec<CalendarEvent>,
}

#[derive(Debug, Deserialize)]
struct RawRemindersResponse {
    total_matched: usize,
    reminders: Vec<ReminderItem>,
}

impl CalendarAdapter {
    pub fn new(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }

    pub fn events(
        &self,
        days: u32,
        calendar: Option<&str>,
        limit: usize,
    ) -> Result<CalendarEventsResult, MacflyError> {
        let payload = serde_json::json!({
            "days": days,
            "calendar": calendar,
            "limit": limit,
        });
        let raw: RawCalendarResponse =
            run_jxa("Apple Calendar", SCRIPT_CALENDAR, &payload, self.timeout_secs)?;
        let returned = raw.events.len();
        Ok(CalendarEventsResult {
            schema_version: "1.0".to_string(),
            days,
            total_matched: raw.total_matched,
            returned,
            events: raw.events,
        })
    }
}

impl RemindersAdapter {
    pub fn new(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }

    pub fn list(
        &self,
        list_filter: Option<&str>,
        overdue_only: bool,
        limit: usize,
    ) -> Result<RemindersListResult, MacflyError> {
        let payload = serde_json::json!({
            "list": list_filter,
            "overdue_only": overdue_only,
            "limit": limit,
        });
        let raw: RawRemindersResponse =
            run_jxa("Apple Reminders", SCRIPT_REMINDERS, &payload, self.timeout_secs)?;
        let returned = raw.reminders.len();
        Ok(RemindersListResult {
            schema_version: "1.0".to_string(),
            total_matched: raw.total_matched,
            returned,
            reminders: raw.reminders,
        })
    }

    pub fn execute_operation(&self, op: &PlanOperation) -> Result<String, MacflyError> {
        let payload = match op {
            PlanOperation::RemindersCreate { list, title, notes } => serde_json::json!({
                "action": "reminders_create",
                "list": list,
                "title": title,
                "notes": notes,
            }),
            _ => {
                return Err(MacflyError::PlanError(
                    "Operation gehört nicht zum Reminders-Adapter".to_string(),
                ));
            }
        };
        let res: RawMutateResponse = run_jxa(
            "Apple Reminders",
            SCRIPT_REMINDERS_MUTATE,
            &payload,
            self.timeout_secs,
        )?;
        if !res.ok {
            return Err(MacflyError::ScriptError(res.detail));
        }
        Ok(res.detail)
    }
}
