use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MacflyError {
    #[error("macOS permission denied (TCC) for {target}: {detail}")]
    TccPermissionDenied { target: String, detail: String },

    #[error("operation blocked by profile policy: {0}")]
    ScopeDenied(String),

    #[error("resource not found: {0}")]
    NotFound(String),

    #[error("backend timed out after {seconds}s")]
    Timeout { seconds: u64 },

    #[error("script execution failed: {0}")]
    ScriptError(String),

    #[error("invalid configuration at {path}: {detail}")]
    ConfigError { path: String, detail: String },

    #[error("plan error: {0}")]
    PlanError(String),

    #[error("plan approval required or invalid: {0}")]
    ApprovalError(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub ok: bool,
    pub error: ErrorBody,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl MacflyError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::TccPermissionDenied { .. } => "TCC_PERMISSION_DENIED",
            Self::ScopeDenied(_) => "SCOPE_DENIED",
            Self::NotFound(_) => "NOT_FOUND",
            Self::Timeout { .. } => "TIMEOUT",
            Self::ScriptError(_) => "SCRIPT_ERROR",
            Self::ConfigError { .. } => "CONFIG_ERROR",
            Self::PlanError(_) => "PLAN_ERROR",
            Self::ApprovalError(_) => "APPROVAL_ERROR",
            Self::Serialization(_) => "SERIALIZATION_ERROR",
            Self::Io(_) => "IO_ERROR",
        }
    }

    pub fn hint(&self) -> Option<String> {
        match self {
            Self::TccPermissionDenied { target, .. } => Some(format!(
                "Erlaube deinem Terminal den Automationszugriff auf {target} unter Systemeinstellungen → Datenschutz & Sicherheit → Automation, und prüfe danach `macfly doctor`."
            )),
            Self::Timeout { .. } => Some(
                "Erhöhe ggf. `script_timeout_secs` in der Config oder begrenze die Abfrage mit `--folder` / `--limit`."
                    .to_string(),
            ),
            _ => None,
        }
    }

    pub fn to_envelope(&self) -> ErrorEnvelope {
        ErrorEnvelope {
            ok: false,
            error: ErrorBody {
                code: self.code(),
                message: self.to_string(),
                hint: self.hint(),
            },
        }
    }
}
