use std::sync::LazyLock;

use regex::Regex;

use crate::model::FidelityInfo;

static BLOCK_BREAK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)</?(?:div|p|br|h[1-6]|tr|section|article)\b[^>]*>").expect("block break regex")
});
static LI_OPEN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<li\b[^>]*>").expect("li open regex"));
static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").expect("tag regex"));
static MULTI_NL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\n{3,}").expect("multi newline regex"));

pub fn analyze_note_fidelity(html: &str, attachment_count: usize) -> FidelityInfo {
    let lower = html.to_ascii_lowercase();
    let has_attachments = attachment_count > 0 || lower.contains("<object");
    let has_tables = lower.contains("<table");
    let has_images = lower.contains("<img");
    let has_internal_links = lower.contains("applenotes://");
    let has_checklists = lower.contains("type=\"checkbox\"")
        || lower.contains("apple-checklist")
        || lower.contains("☐")
        || lower.contains("☑");

    let is_rich =
        has_attachments || has_tables || has_images || has_internal_links || has_checklists;

    let (safe_mutations, blocked_mutations) = if is_rich {
        (
            vec!["append".to_string(), "move".to_string()],
            vec!["replace_body".to_string()],
        )
    } else {
        (
            vec![
                "append".to_string(),
                "move".to_string(),
                "replace_body".to_string(),
            ],
            vec![],
        )
    };

    FidelityInfo {
        has_attachments,
        has_tables,
        has_checklists,
        has_internal_links,
        has_images,
        safe_mutations,
        blocked_mutations,
    }
}

pub fn html_to_clean_text(html: &str) -> String {
    let with_bullets = LI_OPEN_RE.replace_all(html, "\n- ");
    let with_breaks = BLOCK_BREAK_RE.replace_all(&with_bullets, "\n");
    let stripped = TAG_RE.replace_all(&with_breaks, "");
    let decoded = decode_basic_entities(&stripped);

    let normalized_lines: Vec<String> = decoded
        .lines()
        .map(|l| l.trim_end().to_string())
        .collect();
    let joined = normalized_lines.join("\n");
    MULTI_NL_RE
        .replace_all(joined.trim(), "\n\n")
        .into_owned()
}

/// Bereinigt E-Mail-Text: entfernt überlange Zitatketten (`> ...`) und klassische Reply-Trenner,
/// damit KI-Agenten weniger Tokens verbrauchen und fokussiert auf die aktuelle Nachricht bleiben.
pub fn clean_mail_body(raw: &str) -> String {
    let mut out = Vec::new();
    let mut quote_lines_skipped = 0usize;

    for line in raw.lines() {
        let trimmed = line.trim();
        // Klassische Zitat-Blöcke am Ende kompakter halten
        if trimmed.starts_with('>') {
            quote_lines_skipped += 1;
            continue;
        }
        if quote_lines_skipped > 0 {
            out.push(format!("[… {quote_lines_skipped} zitierte Zeilen ausgeblendet]"));
            quote_lines_skipped = 0;
        }
        out.push(line.trim_end().to_string());
    }
    if quote_lines_skipped > 0 {
        out.push(format!("[… {quote_lines_skipped} zitierte Zeilen ausgeblendet]"));
    }

    let joined = out.join("\n");
    MULTI_NL_RE
        .replace_all(joined.trim(), "\n\n")
        .into_owned()
}

fn decode_basic_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_apple_notes_html_cleanly() {
        let html = "<div><h1>Projekt Alpha</h1></div><div><br></div><ul><li>Punkt 1</li><li>Punkt 2 &amp; mehr</li></ul>";
        let clean = html_to_clean_text(html);
        assert!(clean.contains("Projekt Alpha"));
        assert!(clean.contains("- Punkt 1"));
        assert!(clean.contains("- Punkt 2 & mehr"));
    }

    #[test]
    fn protects_rich_notes_from_replace() {
        let html = "<div>Text</div><table><tr><td>A</td></tr></table>";
        let f = analyze_note_fidelity(html, 0);
        assert!(f.has_tables);
        assert!(f.blocked_mutations.iter().any(|m| m == "replace_body"));
        assert!(f.safe_mutations.iter().any(|m| m == "append"));
    }
}
