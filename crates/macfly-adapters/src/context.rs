use macfly_core::config::ProfileConfig;
use macfly_core::model::{
    CalendarEvent, DailyBrief, MeetingPrepContext, PersonContext, ReminderItem, WeeklyReviewContext,
};
use macfly_core::plan::now_unix;

use crate::calendar::{CalendarAdapter, RemindersAdapter};
use crate::contacts::ContactsAdapter;
use crate::mail::{ListMailQuery, MailAdapter};
use crate::notes::{ListNotesQuery, NotesAdapter};
use crate::safari::SafariAdapter;

pub fn build_daily_brief(
    profile_name: &str,
    profile: &ProfileConfig,
    timeout_secs: u64,
    mail_limit: usize,
    notes_limit: usize,
) -> DailyBrief {
    let notes = NotesAdapter::new(timeout_secs);
    let mail = MailAdapter::new(timeout_secs);
    let cal = CalendarAdapter::new(timeout_secs);
    let rem = RemindersAdapter::new(timeout_secs);

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
            limit: mail_limit,
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
            limit: notes_limit,
            ..Default::default()
        },
    ) {
        Ok(n) => n.notes,
        Err(e) => {
            warnings.push(format!("Notes nicht verfügbar: {e}"));
            Vec::new()
        }
    };

    DailyBrief {
        schema_version: "1.0".to_string(),
        profile: profile_name.to_string(),
        generated_at_unix: now_unix(),
        upcoming_events,
        open_reminders,
        unread_mail_count: unread_mails.len(),
        unread_mails,
        recent_notes,
        warnings,
    }
}

pub fn build_person_context(
    profile_name: &str,
    profile: &ProfileConfig,
    timeout_secs: u64,
    query: &str,
    limit: usize,
) -> PersonContext {
    let contacts_adapter = ContactsAdapter::new(timeout_secs);
    let mail_adapter = MailAdapter::new(timeout_secs);
    let notes_adapter = NotesAdapter::new(timeout_secs);
    let cal_adapter = CalendarAdapter::new(timeout_secs);
    let rem_adapter = RemindersAdapter::new(timeout_secs);

    let mut warnings = Vec::new();
    let needle = query.to_lowercase();

    let contacts = match contacts_adapter.list(Some(query), limit) {
        Ok(c) => c.contacts,
        Err(e) => {
            warnings.push(format!("Contacts nicht verfügbar: {e}"));
            Vec::new()
        }
    };

    let recent_mails = match mail_adapter.list(
        profile_name,
        profile,
        &ListMailQuery {
            mailbox: Some("INBOX".to_string()),
            query: Some(query.to_string()),
            limit,
            scan_limit: 250,
            ..Default::default()
        },
    ) {
        Ok(m) => m.messages,
        Err(e) => {
            warnings.push(format!("Mail nicht verfügbar: {e}"));
            Vec::new()
        }
    };

    let matching_notes = match notes_adapter.list(
        profile_name,
        profile,
        &ListNotesQuery {
            query: Some(query.to_string()),
            limit,
            ..Default::default()
        },
    ) {
        Ok(n) => n.notes,
        Err(e) => {
            warnings.push(format!("Notes nicht verfügbar: {e}"));
            Vec::new()
        }
    };

    let upcoming_events: Vec<CalendarEvent> = match cal_adapter.events(30, None, 100) {
        Ok(c) => c
            .events
            .into_iter()
            .filter(|ev| {
                ev.title.to_lowercase().contains(&needle)
                    || ev
                        .location
                        .as_deref()
                        .map(|l| l.to_lowercase().contains(&needle))
                        .unwrap_or(false)
            })
            .take(limit)
            .collect(),
        Err(e) => {
            warnings.push(format!("Calendar nicht verfügbar: {e}"));
            Vec::new()
        }
    };

    let open_reminders: Vec<ReminderItem> = match rem_adapter.list(None, false, 100) {
        Ok(r) => r
            .reminders
            .into_iter()
            .filter(|item| item.title.to_lowercase().contains(&needle))
            .take(limit)
            .collect(),
        Err(e) => {
            warnings.push(format!("Reminders nicht verfügbar: {e}"));
            Vec::new()
        }
    };

    PersonContext {
        schema_version: "1.0".to_string(),
        profile: profile_name.to_string(),
        query: query.to_string(),
        generated_at_unix: now_unix(),
        contacts,
        recent_mails,
        matching_notes,
        upcoming_events,
        open_reminders,
        warnings,
    }
}

pub fn build_meeting_prep(
    profile_name: &str,
    profile: &ProfileConfig,
    timeout_secs: u64,
    query: Option<&str>,
) -> MeetingPrepContext {
    let cal_adapter = CalendarAdapter::new(timeout_secs);
    let mut warnings = Vec::new();

    let events = match cal_adapter.events(14, None, 50) {
        Ok(c) => c.events,
        Err(e) => {
            warnings.push(format!("Calendar nicht verfügbar: {e}"));
            Vec::new()
        }
    };

    let target_event = if let Some(q) = query {
        let needle = q.to_lowercase();
        events
            .into_iter()
            .find(|ev| ev.title.to_lowercase().contains(&needle))
    } else {
        events.into_iter().next()
    };

    let search_term = query
        .map(String::from)
        .or_else(|| target_event.as_ref().map(|ev| ev.title.clone()))
        .unwrap_or_else(|| "Meeting".to_string());

    let person_ctx = build_person_context(profile_name, profile, timeout_secs, &search_term, 10);
    warnings.extend(person_ctx.warnings);

    MeetingPrepContext {
        schema_version: "1.0".to_string(),
        profile: profile_name.to_string(),
        query: search_term,
        generated_at_unix: now_unix(),
        target_event,
        related_contacts: person_ctx.contacts,
        related_mails: person_ctx.recent_mails,
        related_notes: person_ctx.matching_notes,
        warnings,
    }
}

pub fn build_weekly_review(
    profile_name: &str,
    profile: &ProfileConfig,
    timeout_secs: u64,
) -> WeeklyReviewContext {
    let notes = NotesAdapter::new(timeout_secs);
    let mail = MailAdapter::new(timeout_secs);
    let cal = CalendarAdapter::new(timeout_secs);
    let rem = RemindersAdapter::new(timeout_secs);
    let safari = SafariAdapter::new(timeout_secs);

    let mut warnings = Vec::new();

    let recent_notes = match notes.list(
        profile_name,
        profile,
        &ListNotesQuery {
            limit: 25,
            ..Default::default()
        },
    ) {
        Ok(n) => n.notes,
        Err(e) => {
            warnings.push(format!("Notes nicht verfügbar: {e}"));
            Vec::new()
        }
    };

    let unread_mails = match mail.list(
        profile_name,
        profile,
        &ListMailQuery {
            mailbox: Some("INBOX".to_string()),
            unread_only: true,
            limit: 25,
            scan_limit: 200,
            ..Default::default()
        },
    ) {
        Ok(m) => m.messages,
        Err(e) => {
            warnings.push(format!("Mail nicht verfügbar: {e}"));
            Vec::new()
        }
    };

    let overdue_reminders = match rem.list(None, true, 25) {
        Ok(r) => r.reminders,
        Err(e) => {
            warnings.push(format!("Reminders nicht verfügbar: {e}"));
            Vec::new()
        }
    };

    let upcoming_events = match cal.events(7, None, 25) {
        Ok(c) => c.events,
        Err(e) => {
            warnings.push(format!("Calendar nicht verfügbar: {e}"));
            Vec::new()
        }
    };

    let open_safari_tabs = match safari.tabs(None) {
        Ok(t) => t.tabs,
        Err(e) => {
            warnings.push(format!("Safari nicht verfügbar: {e}"));
            Vec::new()
        }
    };

    WeeklyReviewContext {
        schema_version: "1.0".to_string(),
        profile: profile_name.to_string(),
        generated_at_unix: now_unix(),
        recent_notes,
        unread_mails,
        overdue_reminders,
        upcoming_events,
        open_safari_tabs,
        warnings,
    }
}
