use std::collections::HashSet;

use macfly_core::config::ProfileConfig;
use macfly_core::error::MacflyError;
use macfly_core::journal::AuditJournal;
use macfly_core::plan::{
    OperationRecord, PlanOperation, PlanStatus, PlanStore, StoredPlan, now_unix,
    verify_plan_integrity,
};
use serde::Serialize;

use crate::calendar::RemindersAdapter;
use crate::mail::MailAdapter;
use crate::notes::NotesAdapter;
use crate::shortcuts::ShortcutsAdapter;

#[derive(Debug, Serialize)]
pub struct ApplyReport {
    pub plan_id: String,
    pub status: PlanStatus,
    pub total_ops: usize,
    pub succeeded_count: usize,
    pub skipped_count: usize,
    pub results: Vec<String>,
}

pub fn apply_approved_plan(
    store: &PlanStore,
    journal: &AuditJournal,
    plan_id: &str,
    profile_name: &str,
    profile: &ProfileConfig,
    timeout_secs: u64,
    resume: bool,
) -> Result<ApplyReport, MacflyError> {
    let _lock = store.acquire_lock(plan_id)?;
    let mut plan: StoredPlan = store.load(plan_id)?;
    verify_plan_integrity(&plan)?;

    if plan.profile != profile_name {
        return Err(MacflyError::ApprovalError(format!(
            "Plan '{}' wurde für Profil '{}' erstellt, nicht für '{}'",
            plan.plan_id, plan.profile, profile_name
        )));
    }

    let allowed_status = if resume {
        matches!(
            plan.status,
            PlanStatus::Approved | PlanStatus::PartiallyApplied
        )
    } else {
        plan.status == PlanStatus::Approved
    };
    if !allowed_status {
        let hint = if plan.status == PlanStatus::PartiallyApplied {
            format!(
                "Plan '{}' wurde teilweise ausgeführt ({}/{} Operationen). Fortsetzen mit `macfly plan apply {} --resume`.",
                plan.plan_id,
                plan.completed_ops.iter().filter(|r| r.succeeded).count(),
                plan.operations.len(),
                plan.plan_id
            )
        } else {
            format!(
                "Plan '{}' ist nicht freigegeben (aktueller Status: {:?}). Bitte zuerst `macfly plan approve {}` ausführen.",
                plan.plan_id, plan.status, plan.plan_id
            )
        };
        return Err(MacflyError::ApprovalError(hint));
    }

    if !resume && now_unix() > plan.expires_at_unix {
        return Err(MacflyError::ApprovalError(format!(
            "Freigabe für Plan '{}' ist abgelaufen",
            plan.plan_id
        )));
    }

    let already_done: HashSet<usize> = plan
        .completed_ops
        .iter()
        .filter(|r| r.succeeded)
        .map(|r| r.index)
        .collect();

    let notes = NotesAdapter::new(timeout_secs);
    let mail = MailAdapter::new(timeout_secs);
    let shortcuts = ShortcutsAdapter::new(timeout_secs);
    let reminders = RemindersAdapter::new(timeout_secs);

    let total_ops = plan.operations.len();
    let mut skipped_count = 0usize;
    let mut succeeded_count = already_done.len();
    let mut results = Vec::new();

    for (idx, op) in plan.operations.clone().iter().enumerate() {
        if already_done.contains(&idx) {
            skipped_count += 1;
            results.push(format!("[op {idx}] übersprungen (bereits ausgeführt)"));
            continue;
        }

        let outcome = match op {
            PlanOperation::NotesCreate { .. }
            | PlanOperation::NotesAppend { .. }
            | PlanOperation::NotesMove { .. }
            | PlanOperation::NotesMoveFolderContents { .. }
            | PlanOperation::NotesCreateFolder { .. }
            | PlanOperation::NotesDeleteEmptyFolder { .. } => {
                notes.execute_operation(profile_name, profile, op)
            }
            PlanOperation::MailCreateDraft { .. }
            | PlanOperation::MailMarkRead { .. }
            | PlanOperation::MailMove { .. } => mail.execute_operation(profile, op),
            PlanOperation::ShortcutsRun { name, input } => {
                shortcuts.run(profile_name, profile, name, input.as_deref())
            }
            PlanOperation::RemindersCreate { .. } => reminders.execute_operation(op),
        };

        match outcome {
            Ok(msg) => {
                succeeded_count += 1;
                plan.completed_ops.push(OperationRecord {
                    index: idx,
                    succeeded: true,
                    message: msg.clone(),
                    executed_at_unix: now_unix(),
                });
                results.push(msg);
                // Zwischenstand sofort persistieren, damit --resume exakt hier ansetzen kann.
                let _ = store.save(&plan);
            }
            Err(err) => {
                plan.status = if succeeded_count > 0 {
                    PlanStatus::PartiallyApplied
                } else {
                    PlanStatus::Failed
                };
                let _ = store.save(&plan);
                results.push(format!("FEHLER bei op {idx}: {err}"));
                let _ = journal.record_apply(
                    &plan.plan_id,
                    profile_name,
                    &plan.plan_hash,
                    false,
                    results,
                );
                return Err(err);
            }
        }
    }

    plan.status = PlanStatus::Applied;
    plan.applied_at_unix = Some(now_unix());
    store.save(&plan)?;
    journal.record_apply(
        &plan.plan_id,
        profile_name,
        &plan.plan_hash,
        true,
        results.clone(),
    )?;

    Ok(ApplyReport {
        plan_id: plan.plan_id,
        status: plan.status,
        total_ops,
        succeeded_count,
        skipped_count,
        results,
    })
}
