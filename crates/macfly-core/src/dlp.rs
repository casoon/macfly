use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::config::DlpConfig;

struct Rule {
    name: &'static str,
    label: &'static str,
    regex: Regex,
}

static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    vec![
        Rule {
            name: "private_key",
            label: "PRIVATE_KEY",
            regex: Regex::new(
                r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?-----END [A-Z0-9 ]*PRIVATE KEY-----",
            )
            .expect("valid private_key regex"),
        },
        Rule {
            name: "api_key",
            label: "API_KEY",
            regex: Regex::new(
                r"\b(?:sk-[A-Za-z0-9_-]{20,}|gh[pousr]_[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_-]{35}|xox[baprs]-[0-9A-Za-z-]{10,})\b",
            )
            .expect("valid api_key regex"),
        },
        Rule {
            name: "iban",
            label: "IBAN",
            regex: Regex::new(r"\b[A-Z]{2}\d{2}(?:[ ]?[A-Z0-9]{4}){3,7}(?:[ ]?[A-Z0-9]{1,4})?\b")
                .expect("valid iban regex"),
        },
        Rule {
            name: "credit_card",
            label: "CREDIT_CARD",
            regex: Regex::new(r"\b(?:\d{4}[ -]){3}\d{4}\b").expect("valid credit_card regex"),
        },
        Rule {
            name: "otp_code",
            label: "OTP_CODE",
            regex: Regex::new(
                r"(?i)\b(?:otp|tan|2fa|verification code|sicherheitscode|bestätigungscode|code)\s*[:#-]?\s*\d{6}\b",
            )
            .expect("valid otp_code regex"),
        },
    ]
});

#[derive(Debug, Clone)]
pub struct RedactionResult {
    pub text: String,
    pub counts: BTreeMap<String, usize>,
    pub truncated: bool,
}

pub fn apply_dlp(input: &str, cfg: &DlpConfig) -> RedactionResult {
    let mut text = input.to_string();
    let mut counts = BTreeMap::new();

    if cfg.enabled {
        for rule in RULES.iter() {
            if !cfg.redact.iter().any(|r| r.eq_ignore_ascii_case(rule.name)) {
                continue;
            }
            let mut hit_count = 0usize;
            let replaced = rule.regex.replace_all(&text, |_: &regex::Captures<'_>| {
                hit_count += 1;
                format!("[REDACTED:{}_{}]", rule.label, hit_count)
            });
            if hit_count > 0 {
                text = replaced.into_owned();
                counts.insert(rule.name.to_string(), hit_count);
            }
        }
    }

    let mut truncated = false;
    if cfg.max_body_chars > 0 && text.chars().count() > cfg.max_body_chars {
        text = text.chars().take(cfg.max_body_chars).collect::<String>();
        text.push_str("\n… [gekürzt durch max_body_chars]");
        truncated = true;
    }

    RedactionResult {
        text,
        counts,
        truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_iban_api_key_and_otp() {
        let cfg = DlpConfig::default();
        let sample = "Überweise an DE89 3704 0044 0532 0130 00. Key: sk-proj1234567890abcdefghij. Sicherheitscode: 482910.";
        let res = apply_dlp(sample, &cfg);
        assert!(res.text.contains("[REDACTED:IBAN_1]"));
        assert!(res.text.contains("[REDACTED:API_KEY_1]"));
        assert!(res.text.contains("[REDACTED:OTP_CODE_1]"));
        assert_eq!(res.counts.get("iban"), Some(&1));
        assert_eq!(res.counts.get("api_key"), Some(&1));
        assert_eq!(res.counts.get("otp_code"), Some(&1));
        assert!(!res.truncated);
    }

    #[test]
    fn respects_disabled_dlp() {
        let cfg = DlpConfig {
            enabled: false,
            ..DlpConfig::default()
        };
        let sample = "DE89 3704 0044 0532 0130 00";
        let res = apply_dlp(sample, &cfg);
        assert_eq!(res.text, sample);
        assert!(res.counts.is_empty());
    }
}
