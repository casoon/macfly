use macfly_core::config::ProfileConfig;
use macfly_core::error::MacflyError;
use macfly_core::journal::AuditJournal;
use macfly_core::plan::{
    PlanOperation, PlanStatus, PlanStore, StoredPlan, now_unix, verify_plan_integrity,
};
use serde::Serialize;

use crate::mail::MailAdapter;
use crate::notes::NotesAdapter;

#[derive(Debug, Serialize)]
pub struct ApplyReport {
    pub plan_id: String,
    pub status: PlanStatus,
    pub results: Vec<String>,
}

pub fn apply_approved_plan(
    store: &PlanStore,
    journal: &AuditJournal,
    plan_id: &str,
    profile_name: &str,
    profile: &ProfileConfig,
    timeout_secs: u64,
) -> Result<ApplyReport, MacflyError> {
    let mut plan: StoredPlan = store.load(plan_id)?;
    verify_plan_integrity(&plan)?;

    if plan.profile != profile_name {
        return Err(MacflyError::ApprovalError(format!(
            "Plan '{}' wurde für Profil '{}' erstellt, nicht für '{}'",
            plan.plan_id, plan.profile, profile_name
        )));
    }
    if plan.status != PlanStatus::Approved {
        return Err(MacflyError::ApprovalError(format!(
            "Plan '{}' ist nicht freigegeben (aktueller Status: {:?}). Bitte zuerst `macfly plan approve {}` ausführen.",
            plan.plan_id, plan.status, plan.plan_id
        )));
    }
    if now_unix() > plan.expires_at_unix {
        return Err(MacflyError::ApprovalError(format!(
            "Freigabe für Plan '{}' ist abgelaufen",
            plan.plan_id
        )));
    }

    let notes = NotesAdapter::new(timeout_secs);
    let mail = MailAdapter::new(timeout_secs);
    let mut results = Vec::new();

    for op in &plan.operations {
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
        };

        match outcome {
            Ok(msg) => results.push(msg),
            Err(err) => {
                plan.status = PlanStatus::Failed;
                let _ = store.save(&plan);
                results.push(format!("FEHLER: {err}"));
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
        results,
    })
}
