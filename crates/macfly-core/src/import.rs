use std::path::Path;

use serde_json::Value;

use crate::error::MacflyError;
use crate::html::html_to_clean_text;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedNote {
    pub title: String,
    pub body: String,
    pub folder_override: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedReminder {
    pub title: String,
    pub notes: Option<String>,
    pub list_override: Option<String>,
}

/// Unterstützte Dateiendungen für `macfly notes import`.
pub fn is_supported_note_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .as_deref(),
        Some("md" | "markdown" | "txt" | "html" | "htm" | "json" | "jsonl" | "csv")
    )
}

/// Unterstützte Dateiendungen für `macfly reminders import`.
pub fn is_supported_reminder_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .as_deref(),
        Some("md" | "markdown" | "txt" | "json" | "jsonl" | "csv")
    )
}

/// Parst eine Datei anhand ihrer Endung (`.md`, `.txt`, `.html`, `.json`, `.jsonl`, `.csv`)
/// in eine oder mehrere `ImportedNote`-Einträge.
pub fn parse_notes_from_str(path: &Path, raw: &str) -> Result<Vec<ImportedNote>, MacflyError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let fallback_title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Imported Note")
        .to_string();

    match ext.as_str() {
        "json" => parse_notes_json(raw, &fallback_title),
        "jsonl" => parse_notes_jsonl(raw, &fallback_title),
        "csv" => parse_notes_csv(raw),
        "html" | "htm" => Ok(vec![parse_note_html(raw, &fallback_title)]),
        _ => Ok(vec![parse_note_markdown(raw, &fallback_title)]),
    }
}

/// Parst eine Datei anhand ihrer Endung (`.md`, `.txt`, `.json`, `.jsonl`, `.csv`)
/// in `ImportedReminder`-Einträge.
pub fn parse_reminders_from_str(
    path: &Path,
    raw: &str,
) -> Result<Vec<ImportedReminder>, MacflyError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "json" => parse_reminders_json(raw),
        "jsonl" => parse_reminders_jsonl(raw),
        "csv" => parse_reminders_csv(raw),
        "txt" => Ok(parse_reminders_txt(raw)),
        _ => Ok(parse_reminders_markdown(raw)),
    }
}

fn parse_note_markdown(raw: &str, fallback_title: &str) -> ImportedNote {
    let mut body = raw.trim().to_string();
    let mut fm_title: Option<String> = None;
    let mut fm_folder: Option<String> = None;

    if let Some(rest) = body.strip_prefix("---\n")
        && let Some(end_idx) = rest.find("\n---\n")
    {
        let fm = &rest[..end_idx];
        for line in fm.lines() {
            if let Some(val) = line.strip_prefix("title:") {
                let t = unquote(val);
                if !t.is_empty() {
                    fm_title = Some(t);
                }
            } else if let Some(val) = line.strip_prefix("folder:") {
                let f = unquote(val);
                if !f.is_empty() {
                    fm_folder = Some(f);
                }
            }
        }
        body = rest[end_idx + 5..].trim().to_string();
    }

    if let Some(title) = fm_title {
        return ImportedNote {
            title,
            body,
            folder_override: fm_folder,
        };
    }

    let mut lines = body.lines();
    if let Some(first) = lines.next()
        && let Some(heading) = first.strip_prefix("# ")
    {
        let title = heading.trim().to_string();
        let remaining: String = lines.collect::<Vec<_>>().join("\n").trim().to_string();
        if !title.is_empty() {
            return ImportedNote {
                title,
                body: remaining,
                folder_override: fm_folder,
            };
        }
    }

    ImportedNote {
        title: fallback_title.to_string(),
        body,
        folder_override: fm_folder,
    }
}

fn parse_note_html(raw: &str, fallback_title: &str) -> ImportedNote {
    let title = extract_html_tag(raw, "title")
        .or_else(|| extract_html_tag(raw, "h1"))
        .map(|s| html_to_clean_text(&s))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| fallback_title.to_string());
    let body = html_to_clean_text(raw);
    ImportedNote {
        title,
        body,
        folder_override: None,
    }
}

fn extract_html_tag(html: &str, tag: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let open_pat = format!("<{tag}");
    let close_pat = format!("</{tag}>");
    let start_tag = lower.find(&open_pat)?;
    let gt = lower[start_tag..].find('>')? + start_tag + 1;
    let end_tag = lower[gt..].find(&close_pat)? + gt;
    Some(html[gt..end_tag].trim().to_string())
}

fn parse_notes_json(raw: &str, fallback_title: &str) -> Result<Vec<ImportedNote>, MacflyError> {
    let val: Value = serde_json::from_str(raw)?;
    match &val {
        Value::Array(items) => {
            let mut out = Vec::new();
            for (idx, item) in items.iter().enumerate() {
                if let Some(note) =
                    note_from_json_value(item, &format!("{fallback_title} #{}", idx + 1))
                {
                    out.push(note);
                }
            }
            Ok(out)
        }
        Value::Object(map) => {
            if let Some(Value::Array(items)) = map.get("notes").or_else(|| map.get("items")) {
                let mut out = Vec::new();
                for (idx, item) in items.iter().enumerate() {
                    if let Some(note) =
                        note_from_json_value(item, &format!("{fallback_title} #{}", idx + 1))
                    {
                        out.push(note);
                    }
                }
                return Ok(out);
            }
            Ok(note_from_json_value(&val, fallback_title)
                .into_iter()
                .collect())
        }
        _ => Err(MacflyError::PlanError(
            "JSON-Import erwartet ein Objekt oder ein Array von Notizen".to_string(),
        )),
    }
}

fn parse_notes_jsonl(raw: &str, fallback_title: &str) -> Result<Vec<ImportedNote>, MacflyError> {
    let mut out = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let val: Value = serde_json::from_str(trimmed)?;
        if let Some(note) = note_from_json_value(&val, &format!("{fallback_title} #{}", idx + 1)) {
            out.push(note);
        }
    }
    Ok(out)
}

fn note_from_json_value(val: &Value, fallback_title: &str) -> Option<ImportedNote> {
    let obj = val.as_object()?;
    let title = pick_str(obj, &["title", "name", "subject", "heading"])
        .unwrap_or_else(|| fallback_title.to_string());
    let body = pick_str(
        obj,
        &[
            "body",
            "body_text",
            "content",
            "text",
            "markdown",
            "note",
            "description",
        ],
    )
    .unwrap_or_default();
    let folder_override = pick_str(obj, &["folder", "folder_path"]);
    if title.is_empty() && body.is_empty() {
        return None;
    }
    Some(ImportedNote {
        title,
        body,
        folder_override,
    })
}

fn parse_notes_csv(raw: &str) -> Result<Vec<ImportedNote>, MacflyError> {
    let rows = parse_csv_rows(raw);
    if rows.len() < 2 {
        return Ok(Vec::new());
    }
    let headers: Vec<String> = rows[0]
        .iter()
        .map(|h| h.trim().to_ascii_lowercase())
        .collect();
    let title_idx = find_col(&headers, &["title", "name", "subject", "heading"]).unwrap_or(0);
    let body_idx = find_col(
        &headers,
        &[
            "body",
            "content",
            "text",
            "notes",
            "note",
            "description",
            "markdown",
        ],
    )
    .or(if headers.len() > 1 { Some(1) } else { None });
    let folder_idx = find_col(&headers, &["folder", "folder_path", "category"]);

    let mut out = Vec::new();
    for row in rows.iter().skip(1) {
        let title = row
            .get(title_idx)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let body = body_idx
            .and_then(|i| row.get(i))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let folder_override = folder_idx
            .and_then(|i| row.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        if title.is_empty() && body.is_empty() {
            continue;
        }
        out.push(ImportedNote {
            title: if title.is_empty() {
                "Imported Note".to_string()
            } else {
                title
            },
            body,
            folder_override,
        });
    }
    Ok(out)
}

fn parse_reminders_markdown(raw: &str) -> Vec<ImportedReminder> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        let item = trimmed
            .strip_prefix("- [ ] ")
            .or_else(|| trimmed.strip_prefix("* [ ] "));
        if let Some(task) = item {
            let title = task.trim();
            if !title.is_empty() {
                out.push(ImportedReminder {
                    title: title.to_string(),
                    notes: None,
                    list_override: None,
                });
            }
        }
    }
    out
}

fn parse_reminders_txt(raw: &str) -> Vec<ImportedReminder> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let clean = trimmed
            .strip_prefix("- [ ] ")
            .or_else(|| trimmed.strip_prefix("* [ ] "))
            .or_else(|| trimmed.strip_prefix("- "))
            .or_else(|| trimmed.strip_prefix("* "))
            .unwrap_or(trimmed)
            .trim();
        if !clean.is_empty() {
            out.push(ImportedReminder {
                title: clean.to_string(),
                notes: None,
                list_override: None,
            });
        }
    }
    out
}

fn parse_reminders_json(raw: &str) -> Result<Vec<ImportedReminder>, MacflyError> {
    let val: Value = serde_json::from_str(raw)?;
    match &val {
        Value::Array(items) => Ok(items.iter().filter_map(reminder_from_json_value).collect()),
        Value::Object(map) => {
            if let Some(Value::Array(items)) = map
                .get("reminders")
                .or_else(|| map.get("tasks"))
                .or_else(|| map.get("items"))
            {
                return Ok(items.iter().filter_map(reminder_from_json_value).collect());
            }
            Ok(reminder_from_json_value(&val).into_iter().collect())
        }
        _ => Err(MacflyError::PlanError(
            "JSON-Import für Reminders erwartet ein Array oder Objekt".to_string(),
        )),
    }
}

fn parse_reminders_jsonl(raw: &str) -> Result<Vec<ImportedReminder>, MacflyError> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let val: Value = serde_json::from_str(trimmed)?;
        if let Some(r) = reminder_from_json_value(&val) {
            out.push(r);
        }
    }
    Ok(out)
}

fn reminder_from_json_value(val: &Value) -> Option<ImportedReminder> {
    if let Some(s) = val.as_str() {
        let t = s.trim();
        if t.is_empty() {
            return None;
        }
        return Some(ImportedReminder {
            title: t.to_string(),
            notes: None,
            list_override: None,
        });
    }
    let obj = val.as_object()?;
    let title = pick_str(obj, &["title", "name", "task", "content", "summary"])?;
    if title.is_empty() {
        return None;
    }
    let notes = pick_str(obj, &["notes", "body", "description", "comment"]);
    let list_override = pick_str(obj, &["list", "project", "category"]);
    Some(ImportedReminder {
        title,
        notes,
        list_override,
    })
}

fn parse_reminders_csv(raw: &str) -> Result<Vec<ImportedReminder>, MacflyError> {
    let rows = parse_csv_rows(raw);
    if rows.len() < 2 {
        return Ok(Vec::new());
    }
    let headers: Vec<String> = rows[0]
        .iter()
        .map(|h| h.trim().to_ascii_lowercase())
        .collect();
    let title_idx =
        find_col(&headers, &["title", "task", "name", "content", "summary"]).unwrap_or(0);
    let notes_idx = find_col(&headers, &["notes", "description", "body", "comment"]);
    let list_idx = find_col(&headers, &["list", "project", "category"]);

    let mut out = Vec::new();
    for row in rows.iter().skip(1) {
        let title = row
            .get(title_idx)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        let notes = notes_idx
            .and_then(|i| row.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let list_override = list_idx
            .and_then(|i| row.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        out.push(ImportedReminder {
            title,
            notes,
            list_override,
        });
    }
    Ok(out)
}

/// Schlanker RFC-4180-CSV-Parser (unterstützt Anführungszeichen, `""`-Escapes und Zeilenumbrüche in Feldern).
pub fn parse_csv_rows(input: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
        } else {
            match c {
                '"' => in_quotes = true,
                ',' => {
                    row.push(std::mem::take(&mut field));
                }
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    row.push(std::mem::take(&mut field));
                    if row.iter().any(|cell| !cell.trim().is_empty()) {
                        rows.push(std::mem::take(&mut row));
                    } else {
                        row.clear();
                    }
                }
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    if row.iter().any(|cell| !cell.trim().is_empty()) {
                        rows.push(std::mem::take(&mut row));
                    } else {
                        row.clear();
                    }
                }
                _ => field.push(c),
            }
        }
    }

    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        if row.iter().any(|cell| !cell.trim().is_empty()) {
            rows.push(row);
        }
    }

    rows
}

fn find_col(headers: &[String], candidates: &[&str]) -> Option<usize> {
    for cand in candidates {
        if let Some(pos) = headers.iter().position(|h| h == cand) {
            return Some(pos);
        }
    }
    None
}

fn pick_str(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(v) = map.get(*k).and_then(Value::as_str) {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn unquote(val: &str) -> String {
    val.trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_csv_json_jsonl_and_html_notes() {
        let csv = "title,content,folder\n\"Alpha, 1\",\"Line 1\nLine 2\",10 · Work\nBeta,Hello,\n";
        let notes = parse_notes_from_str(Path::new("export.csv"), csv).unwrap();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].title, "Alpha, 1");
        assert_eq!(notes[0].body, "Line 1\nLine 2");
        assert_eq!(notes[0].folder_override.as_deref(), Some("10 · Work"));

        let jsonl = "{\"name\":\"N1\",\"text\":\"B1\"}\n{\"title\":\"N2\",\"body\":\"B2\"}\n";
        let notes_jl = parse_notes_from_str(Path::new("notes.jsonl"), jsonl).unwrap();
        assert_eq!(notes_jl.len(), 2);
        assert_eq!(notes_jl[0].title, "N1");
        assert_eq!(notes_jl[1].body, "B2");

        let html = "<html><head><title>Evernote Clip</title></head><body><p>Saved paragraph</p></body></html>";
        let notes_html = parse_notes_from_str(Path::new("clip.html"), html).unwrap();
        assert_eq!(notes_html[0].title, "Evernote Clip");
        assert!(notes_html[0].body.contains("Saved paragraph"));
    }

    #[test]
    fn parses_csv_and_json_reminders() {
        let csv = "Task,Description,Project\nSteuererklärung,Q4 Belege,Finanzen\n";
        let rems = parse_reminders_from_str(Path::new("tasks.csv"), csv).unwrap();
        assert_eq!(rems.len(), 1);
        assert_eq!(rems[0].title, "Steuererklärung");
        assert_eq!(rems[0].notes.as_deref(), Some("Q4 Belege"));
        assert_eq!(rems[0].list_override.as_deref(), Some("Finanzen"));
    }
}
