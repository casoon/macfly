use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use macfly_core::error::MacflyError;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Führt ein statisches JXA-Skript sicher über `osascript -l JavaScript` aus.
/// Alle Parameter werden ausschließlich als JSON über `stdin` übergeben (kein `argv`-Leakage).
pub fn run_jxa<I, O>(
    target_app: &str,
    script: &str,
    input: &I,
    timeout_secs: u64,
) -> Result<O, MacflyError>
where
    I: Serialize,
    O: DeserializeOwned,
{
    let payload = serde_json::to_vec(input)?;

    let mut child = Command::new("osascript")
        .arg("-l")
        .arg("JavaScript")
        .arg("-e")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            MacflyError::ScriptError(format!("osascript konnte nicht gestartet werden: {e}"))
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&payload)?;
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

    let stdout_bytes = stdout_handle.join().unwrap_or_default();
    let stderr_bytes = stderr_handle.join().unwrap_or_default();
    let stdout = String::from_utf8_lossy(&stdout_bytes).trim().to_string();
    let stderr = String::from_utf8_lossy(&stderr_bytes).trim().to_string();

    if !status.success() {
        let lower = stderr.to_ascii_lowercase();
        if lower.contains("-1743")
            || lower.contains("not authorized to send apple events")
            || lower.contains("nicht berechtigt")
        {
            return Err(MacflyError::TccPermissionDenied {
                target: target_app.to_string(),
                detail: stderr,
            });
        }
        return Err(MacflyError::ScriptError(if stderr.is_empty() {
            format!("osascript exited with status {status}")
        } else {
            stderr
        }));
    }

    serde_json::from_str(&stdout).map_err(|e| {
        MacflyError::ScriptError(format!(
            "Ungültige JSON-Antwort vom JXA-Skript ({e}): {stdout}"
        ))
    })
}
