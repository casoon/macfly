use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::MacflyError;

const LABEL_PREFIX: &str = "com.casoon.macfly.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleJobInfo {
    pub name: String,
    pub label: String,
    pub plist_path: String,
}

pub fn launch_agents_dir() -> Result<PathBuf, MacflyError> {
    let home = dirs::home_dir().ok_or_else(|| {
        MacflyError::ScriptError("Home-Verzeichnis konnte nicht ermittelt werden".to_string())
    })?;
    Ok(home.join("Library").join("LaunchAgents"))
}

pub fn job_label(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    format!("{LABEL_PREFIX}{clean}")
}

/// Erzeugt eine valide macOS `launchd` `.plist`-XML-Datei für zeitgesteuerte lokale MacFly-Routinen.
pub fn render_launchd_plist(
    label: &str,
    macfly_bin: &str,
    args: &[String],
    hour: u8,
    minute: u8,
    log_dir: &str,
) -> String {
    let mut prog_args = format!("    <string>{}</string>\n", escape_xml(macfly_bin));
    for a in args {
        prog_args.push_str(&format!("    <string>{}</string>\n", escape_xml(a)));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
{prog_args}  </array>
  <key>StartCalendarInterval</key>
  <dict>
    <key>Hour</key>
    <integer>{hour}</integer>
    <key>Minute</key>
    <integer>{minute}</integer>
  </dict>
  <key>StandardOutPath</key>
  <string>{log_dir}/{label}.out.log</string>
  <key>StandardErrorPath</key>
  <string>{log_dir}/{label}.err.log</string>
  <key>RunAtLoad</key>
  <false/>
</dict>
</plist>
"#
    )
}

pub fn list_installed_jobs() -> Result<Vec<ScheduleJobInfo>, MacflyError> {
    let dir = launch_agents_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut jobs = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(fname) = path.file_name().and_then(|s| s.to_str())
            && let Some(rest) = fname.strip_prefix(LABEL_PREFIX)
            && let Some(name) = rest.strip_suffix(".plist")
        {
            jobs.push(ScheduleJobInfo {
                name: name.to_string(),
                label: format!("{LABEL_PREFIX}{name}"),
                plist_path: path.display().to_string(),
            });
        }
    }
    jobs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(jobs)
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_valid_launchd_plist() {
        let label = job_label("morning-brief");
        assert_eq!(label, "com.casoon.macfly.morning-brief");
        let xml = render_launchd_plist(
            &label,
            "/Users/test/.cargo/bin/macfly",
            &["context".to_string(), "daily-brief".to_string()],
            7,
            30,
            "/Users/test/.local/share/macfly/logs",
        );
        assert!(xml.contains("<string>com.casoon.macfly.morning-brief</string>"));
        assert!(xml.contains("<integer>7</integer>"));
        assert!(xml.contains("<integer>30</integer>"));
    }
}
