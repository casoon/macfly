use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use macfly_core::config::ProfileConfig;
use macfly_core::error::MacflyError;
use macfly_core::model::{ShortcutSummary, ShortcutsListResult};

pub struct ShortcutsAdapter {
    timeout_secs: u64,
}

impl ShortcutsAdapter {
    pub fn new(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }

    pub fn list(
        &self,
        profile_name: &str,
        profile: &ProfileConfig,
    ) -> Result<ShortcutsListResult, MacflyError> {
        let stdout = run_shortcuts_cmd(&["list"], None, self.timeout_secs)?;
        let mut shortcuts: Vec<ShortcutSummary> = stdout
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|name| ShortcutSummary {
                name: name.to_string(),
                allowed_by_policy: profile.shortcuts.is_allowed(name),
            })
            .collect();
        shortcuts.sort_by_key(|a| a.name.to_lowercase());

        let total = shortcuts.len();
        let allowed_count = shortcuts.iter().filter(|s| s.allowed_by_policy).count();

        Ok(ShortcutsListResult {
            schema_version: "1.0".to_string(),
            profile: profile_name.to_string(),
            total,
            allowed_count,
            shortcuts,
        })
    }

    pub fn run(
        &self,
        profile_name: &str,
        profile: &ProfileConfig,
        name: &str,
        input: Option<&str>,
    ) -> Result<String, MacflyError> {
        if !profile.shortcuts.is_allowed(name) {
            return Err(MacflyError::ScopeDenied(format!(
                "Kurzbefehl '{name}' ist im Profil '{profile_name}' nicht in `shortcuts.allow` freigegeben"
            )));
        }

        let mut args = vec!["run", name];
        if input.is_some() {
            args.push("--input-path");
            args.push("-");
        }
        let stdout = run_shortcuts_cmd(&args, input, self.timeout_secs)?;
        let trimmed = stdout.trim();
        if trimmed.is_empty() {
            Ok(format!("Kurzbefehl '{name}' ausgeführt"))
        } else {
            Ok(format!("Kurzbefehl '{name}' ausgeführt: {trimmed}"))
        }
    }
}

fn run_shortcuts_cmd(
    args: &[&str],
    stdin_data: Option<&str>,
    timeout_secs: u64,
) -> Result<String, MacflyError> {
    let mut child = Command::new("shortcuts")
        .args(args)
        .stdin(if stdin_data.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            MacflyError::ScriptError(format!("macOS `shortcuts` CLI nicht verfügbar: {e}"))
        })?;

    if let (Some(data), Some(mut stdin)) = (stdin_data, child.stdin.take()) {
        stdin.write_all(data.as_bytes())?;
    }

    let mut stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| MacflyError::ScriptError("stdout pipe fehlte".to_string()))?;
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| MacflyError::ScriptError("stderr pipe fehlte".to_string()))?;

    let stdout_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut buf);
        buf
    });
    let stderr_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf);
        buf
    });

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let status = loop {
        match child.try_wait()? {
            Some(status) => break status,
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(MacflyError::Timeout {
                        seconds: timeout_secs,
                    });
                }
                thread::sleep(Duration::from_millis(25));
            }
        }
    };

    let stdout = String::from_utf8_lossy(&stdout_handle.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&stderr_handle.join().unwrap_or_default())
        .trim()
        .to_string();

    if !status.success() {
        return Err(MacflyError::ScriptError(if stderr.is_empty() {
            format!("shortcuts exited with status {status}")
        } else {
            stderr
        }));
    }

    Ok(stdout)
}
