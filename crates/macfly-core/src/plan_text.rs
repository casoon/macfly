//! Verständliche, zweisprachige Beschreibung von Änderungsplänen (Deutsch/Englisch).
//!
//! Gleichartige Schritte hintereinander werden zusammengefasst („15 Ordner verschieben“),
//! damit man den Ablauf erkennt. Inhalte, die der Mensch freigibt (Mail-Entwürfe, Notiztexte,
//! Kurzbefehle), werden nie zusammengefasst und immer vollständig gezeigt.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::plan::{PlanOperation, Reversibility, RiskLevel, SidecarFormat};

/// Sprache der Ausgaben; Standard Englisch, per Config (`language`) oder `MACFLY_LANG` umstellbar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    De,
    #[default]
    En,
}

static CONFIGURED: OnceLock<Lang> = OnceLock::new();

impl Lang {
    /// `de…` oder `en…` (z. B. `de_DE.UTF-8`, `en-US`).
    pub fn parse(value: &str) -> Option<Self> {
        let v = value.trim().to_ascii_lowercase();
        if v.starts_with("de") {
            Some(Self::De)
        } else if v.starts_with("en") {
            Some(Self::En)
        } else {
            None
        }
    }

    /// Merkt sich die Sprache aus der Config für den gesamten Prozess.
    pub fn set_configured(lang: Lang) {
        let _ = CONFIGURED.set(lang);
    }

    /// `MACFLY_LANG`, dann die Sprache aus der Config, sonst Englisch.
    pub fn detect() -> Self {
        std::env::var("MACFLY_LANG")
            .ok()
            .and_then(|v| Self::parse(&v))
            .or_else(|| CONFIGURED.get().copied())
            .unwrap_or_default()
    }

    pub fn t(self, de: &str, en: &str) -> String {
        match self {
            Self::De => de.to_string(),
            Self::En => en.to_string(),
        }
    }
}

pub fn risk_label(risk: RiskLevel, lang: Lang) -> String {
    match risk {
        RiskLevel::Low => lang.t("niedrig", "low"),
        RiskLevel::Medium => lang.t("mittel", "medium"),
        RiskLevel::High => lang.t("hoch", "high"),
    }
}

pub fn undo_label(rev: Reversibility, lang: Lang) -> String {
    match rev {
        Reversibility::Reversible => lang.t("ja", "yes"),
        Reversibility::BestEffort => lang.t("teilweise", "partly"),
        Reversibility::Irreversible => lang.t("nein", "no"),
    }
}

/// Ein Schritt im Ablauf: eine Operation oder mehrere gleichartige hintereinander.
#[derive(Debug, Clone)]
pub struct PlanStep {
    pub title: String,
    /// Zusatzzeilen (Beispiele, Ziel, Inhalt zum Prüfen).
    pub notes: Vec<String>,
    /// Indizes der Operationen in `plan.operations`.
    pub ops: Range<usize>,
    pub high_risk: bool,
}

const MAX_EXAMPLES: usize = 3;

pub fn plan_steps(ops: &[PlanOperation], lang: Lang) -> Vec<PlanStep> {
    let mut steps = Vec::new();
    let mut i = 0;
    while i < ops.len() {
        let key = group_key(&ops[i]);
        let mut j = i + 1;
        if key.is_some() {
            while j < ops.len() && group_key(&ops[j]) == key {
                j += 1;
            }
        }
        let slice = &ops[i..j];
        let (title, notes) = describe(slice, lang);
        steps.push(PlanStep {
            title,
            notes,
            ops: i..j,
            high_risk: slice.iter().any(|o| o.risk_level() == RiskLevel::High),
        });
        i = j;
    }
    steps
}

/// Nur Aufräum-Operationen ohne eigenen Inhalt werden zusammengefasst.
fn group_key(op: &PlanOperation) -> Option<String> {
    match op {
        PlanOperation::NotesMove { .. } => Some("notes_move".to_string()),
        PlanOperation::NotesMoveFolderContents { .. } => Some("notes_move_folder".to_string()),
        PlanOperation::NotesCreateFolder { .. } => Some("notes_create_folder".to_string()),
        PlanOperation::NotesDeleteEmptyFolder { .. } => Some("notes_delete_folder".to_string()),
        PlanOperation::MailMarkRead { read, .. } => Some(format!("mail_mark_read_{read}")),
        PlanOperation::MailMove { .. } => Some("mail_move".to_string()),
        PlanOperation::RemindersCreate { .. } => Some("reminders_create".to_string()),
        _ => None,
    }
}

fn limited(lang: Lang, mut items: Vec<String>) -> Vec<String> {
    let rest = items.len().saturating_sub(MAX_EXAMPLES);
    items.truncate(MAX_EXAMPLES);
    if rest > 0 {
        items.push(match lang {
            Lang::De => format!("… und {rest} weitere"),
            Lang::En => format!("… and {rest} more"),
        });
    }
    items
}

fn counted(items: impl Iterator<Item = String>) -> Vec<(String, usize)> {
    let mut map: BTreeMap<String, usize> = BTreeMap::new();
    for item in items {
        *map.entry(item).or_insert(0) += 1;
    }
    map.into_iter().collect()
}

fn describe(slice: &[PlanOperation], lang: Lang) -> (String, Vec<String>) {
    let n = slice.len();
    let first = &slice[0];
    match first {
        PlanOperation::NotesMove { .. } => {
            let dests = counted(slice.iter().filter_map(|o| match o {
                PlanOperation::NotesMove {
                    destination_folder, ..
                } => Some(destination_folder.clone()),
                _ => None,
            }));
            let lines = dests
                .into_iter()
                .map(|(d, k)| format!("{k}× → '{d}'"))
                .collect();
            (
                match lang {
                    Lang::De if n == 1 => "1 Notiz verschieben".to_string(),
                    Lang::De => format!("{n} Notizen verschieben"),
                    Lang::En if n == 1 => "Move 1 note".to_string(),
                    Lang::En => format!("Move {n} notes"),
                },
                limited(lang, lines),
            )
        }
        PlanOperation::NotesMoveFolderContents { .. } => {
            let lines = slice
                .iter()
                .filter_map(|o| match o {
                    PlanOperation::NotesMoveFolderContents {
                        source_folder_path,
                        destination_folder_path,
                        ..
                    } => Some(format!(
                        "'{source_folder_path}' → '{destination_folder_path}'"
                    )),
                    _ => None,
                })
                .collect();
            (
                match lang {
                    Lang::De if n == 1 => "Notizen eines Ordners verschieben".to_string(),
                    Lang::De => format!("Notizen aus {n} Ordnern verschieben"),
                    Lang::En if n == 1 => "Move the notes of one folder".to_string(),
                    Lang::En => format!("Move the notes of {n} folders"),
                },
                limited(lang, lines),
            )
        }
        PlanOperation::NotesCreateFolder { .. } => {
            let lines = slice
                .iter()
                .filter_map(|o| match o {
                    PlanOperation::NotesCreateFolder {
                        name,
                        parent_folder,
                        ..
                    } => Some(match parent_folder {
                        Some(p) => format!("'{p}/{name}'"),
                        None => format!("'{name}'"),
                    }),
                    _ => None,
                })
                .collect();
            (
                match lang {
                    Lang::De if n == 1 => "1 Ordner anlegen".to_string(),
                    Lang::De => format!("{n} Ordner anlegen"),
                    Lang::En if n == 1 => "Create 1 folder".to_string(),
                    Lang::En => format!("Create {n} folders"),
                },
                limited(lang, lines),
            )
        }
        PlanOperation::NotesDeleteEmptyFolder { .. } => {
            let lines = slice
                .iter()
                .filter_map(|o| match o {
                    PlanOperation::NotesDeleteEmptyFolder { folder_name, .. } => {
                        Some(format!("'{folder_name}'"))
                    }
                    _ => None,
                })
                .collect();
            (
                match lang {
                    Lang::De if n == 1 => "1 leeren Ordner löschen".to_string(),
                    Lang::De => format!("{n} leere Ordner löschen"),
                    Lang::En if n == 1 => "Delete 1 empty folder".to_string(),
                    Lang::En => format!("Delete {n} empty folders"),
                },
                limited(lang, lines),
            )
        }
        PlanOperation::MailMarkRead { read, .. } => {
            let boxes = counted(slice.iter().filter_map(|o| match o {
                PlanOperation::MailMarkRead { mailbox, .. } => Some(mailbox.clone()),
                _ => None,
            }));
            let lines = boxes
                .into_iter()
                .map(|(b, k)| format!("{k}× in '{b}'"))
                .collect();
            let state = if *read {
                lang.t("gelesen", "read")
            } else {
                lang.t("ungelesen", "unread")
            };
            (
                match lang {
                    Lang::De if n == 1 => format!("1 Mail als {state} markieren"),
                    Lang::De => format!("{n} Mails als {state} markieren"),
                    Lang::En if n == 1 => format!("Mark 1 mail as {state}"),
                    Lang::En => format!("Mark {n} mails as {state}"),
                },
                limited(lang, lines),
            )
        }
        PlanOperation::MailMove { .. } => {
            let pairs = counted(slice.iter().filter_map(|o| match o {
                PlanOperation::MailMove {
                    from_mailbox,
                    to_mailbox,
                    ..
                } => Some(format!("'{from_mailbox}' → '{to_mailbox}'")),
                _ => None,
            }));
            let lines = pairs
                .into_iter()
                .map(|(p, k)| format!("{k}× {p}"))
                .collect();
            (
                match lang {
                    Lang::De if n == 1 => "1 Mail verschieben".to_string(),
                    Lang::De => format!("{n} Mails verschieben"),
                    Lang::En if n == 1 => "Move 1 mail".to_string(),
                    Lang::En => format!("Move {n} mails"),
                },
                limited(lang, lines),
            )
        }
        PlanOperation::RemindersCreate { .. } => {
            let lines = slice
                .iter()
                .filter_map(|o| match o {
                    PlanOperation::RemindersCreate { title, .. } => Some(format!("'{title}'")),
                    _ => None,
                })
                .collect();
            (
                match lang {
                    Lang::De if n == 1 => "1 Erinnerung anlegen".to_string(),
                    Lang::De => format!("{n} Erinnerungen anlegen"),
                    Lang::En if n == 1 => "Create 1 reminder".to_string(),
                    Lang::En => format!("Create {n} reminders"),
                },
                limited(lang, lines),
            )
        }
        PlanOperation::MusicPlaylistAdd {
            playlist,
            track_ids,
        } => (
            match lang {
                Lang::De => format!(
                    "{} Titel zur Playlist '{playlist}' hinzufügen",
                    track_ids.len()
                ),
                Lang::En => format!("Add {} tracks to playlist '{playlist}'", track_ids.len()),
            },
            Vec::new(),
        ),
        PlanOperation::PhotosExport {
            media_ids,
            destination,
            originals,
            sidecar,
            convert,
            label,
        } => {
            let count = media_ids.len();
            let from = label
                .as_ref()
                .map(|l| format!(" ({l})"))
                .unwrap_or_default();
            let title = match lang {
                Lang::De if count == 1 => format!("1 Foto exportieren{from}"),
                Lang::De => format!("{count} Fotos exportieren{from}"),
                Lang::En if count == 1 => format!("Export 1 photo{from}"),
                Lang::En => format!("Export {count} photos{from}"),
            };
            let mut notes = vec![match lang {
                Lang::De => format!("Ziel: {destination}"),
                Lang::En => format!("To: {destination}"),
            }];
            notes.push(if *originals {
                lang.t("Originaldateien", "Original files")
            } else {
                lang.t(
                    "Gerenderte JPEGs (mit Bearbeitungen)",
                    "Rendered JPEGs (with edits)",
                )
            });
            match sidecar {
                SidecarFormat::None => {}
                SidecarFormat::Xmp => notes.push(lang.t(
                    "Dazu je Foto eine XMP-Datei mit Titel, Datum, Ort …",
                    "Plus one XMP file per photo with title, date, location …",
                )),
                SidecarFormat::Json => notes.push(lang.t(
                    "Dazu je Foto eine JSON-Datei mit Titel, Datum, Ort …",
                    "Plus one JSON file per photo with title, date, location …",
                )),
                SidecarFormat::Both => notes.push(lang.t(
                    "Dazu je Foto eine XMP- und eine JSON-Datei mit Titel, Datum, Ort …",
                    "Plus an XMP and a JSON file per photo with title, date, location …",
                )),
            }
            if let Some(c) = convert {
                let mut line = format!("{:?}", c.format).to_uppercase();
                if let Some(m) = c.max_size {
                    line.push_str(&match lang {
                        Lang::De => format!(", höchstens {m} px"),
                        Lang::En => format!(", at most {m} px"),
                    });
                }
                if let Some(q) = c.quality {
                    line.push_str(&match lang {
                        Lang::De => format!(", Qualität {q}"),
                        Lang::En => format!(", quality {q}"),
                    });
                }
                notes.push(match lang {
                    Lang::De => format!("Umwandeln in {line}"),
                    Lang::En => format!("Convert to {line}"),
                });
            }
            notes.push(lang.t(
                "Nichts wird überschrieben, deine Mediathek in Fotos bleibt unverändert.",
                "Nothing is overwritten, your Photos library stays unchanged.",
            ));
            (title, notes)
        }
        // Inhalte zum Prüfen: nie zusammenfassen, Details immer zeigen.
        PlanOperation::MailCreateDraft { to, .. } => (
            match lang {
                Lang::De => format!(
                    "Mail-Entwurf an {} anlegen (wird nicht gesendet)",
                    to.join(", ")
                ),
                Lang::En => format!("Create a mail draft to {} (not sent)", to.join(", ")),
            },
            content_lines(first),
        ),
        PlanOperation::NotesCreate { title, folder, .. } => (
            match lang {
                Lang::De => format!("Notiz '{title}' in '{folder}' anlegen"),
                Lang::En => format!("Create note '{title}' in '{folder}'"),
            },
            content_lines(first),
        ),
        PlanOperation::NotesAppend { .. } => (
            lang.t("Text an eine Notiz anhängen", "Append text to a note"),
            content_lines(first),
        ),
        PlanOperation::ShortcutsRun { name, .. } => (
            match lang {
                Lang::De => format!("Kurzbefehl '{name}' ausführen (nicht rückgängig zu machen)"),
                Lang::En => format!("Run shortcut '{name}' (cannot be undone)"),
            },
            content_lines(first),
        ),
    }
}

/// Die Detailzeilen der Operation ohne die Kopfzeile.
fn content_lines(op: &PlanOperation) -> Vec<String> {
    op.detailed_preview()
        .into_iter()
        .skip(1)
        .map(|l| l.strip_prefix("    ").unwrap_or(&l).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{ImageFormat, PhotoConvert};

    fn folder_move(i: usize) -> PlanOperation {
        PlanOperation::NotesMoveFolderContents {
            source_folder_id: format!("id{i}"),
            source_folder_path: format!("Alt {i}"),
            destination_folder_path: "Neu".to_string(),
            destination_account: None,
        }
    }

    #[test]
    fn serial_operations_collapse_into_one_step_with_few_examples() {
        let ops: Vec<_> = (0..15).map(folder_move).collect();
        let steps = plan_steps(&ops, Lang::De);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].title, "Notizen aus 15 Ordnern verschieben");
        assert_eq!(steps[0].ops, 0..15);
        assert_eq!(steps[0].notes.len(), 4);
        assert_eq!(steps[0].notes[3], "… und 12 weitere");
        let en = plan_steps(&ops, Lang::En);
        assert_eq!(en[0].title, "Move the notes of 15 folders");
        assert_eq!(en[0].notes[3], "… and 12 more");
    }

    #[test]
    fn different_kinds_stay_separate_and_keep_order() {
        let ops = vec![
            folder_move(1),
            folder_move(2),
            PlanOperation::NotesCreateFolder {
                account: None,
                name: "X".to_string(),
                parent_folder: None,
            },
            folder_move(3),
        ];
        let steps = plan_steps(&ops, Lang::En);
        let ranges: Vec<_> = steps.iter().map(|s| s.ops.clone()).collect();
        assert_eq!(ranges, vec![0..2, 2..3, 3..4]);
    }

    #[test]
    fn reviewable_content_is_never_collapsed() {
        let draft = |subject: &str| PlanOperation::MailCreateDraft {
            account: None,
            to: vec!["a@b.example".to_string()],
            cc: vec![],
            subject: subject.to_string(),
            body: "Hallo Welt".to_string(),
            signature_name: None,
            reply_to_local_id: None,
        };
        let steps = plan_steps(&[draft("1"), draft("2")], Lang::De);
        assert_eq!(steps.len(), 2);
        assert!(steps[0].notes.iter().any(|l| l.contains("| Hallo Welt")));
    }

    #[test]
    fn photo_step_says_how_many_and_what_happens() {
        let op = PlanOperation::PhotosExport {
            media_ids: (0..200).map(|i| i.to_string()).collect(),
            destination: "/Users/x/Export".to_string(),
            originals: true,
            sidecar: SidecarFormat::Both,
            convert: Some(PhotoConvert {
                format: ImageFormat::Jpeg,
                max_size: Some(2048),
                quality: Some(85),
            }),
            label: Some("Album 'Belege'".to_string()),
        };
        let de = plan_steps(std::slice::from_ref(&op), Lang::De);
        assert_eq!(de[0].title, "200 Fotos exportieren (Album 'Belege')");
        assert!(de[0].notes.contains(&"Ziel: /Users/x/Export".to_string()));
        assert!(
            de[0]
                .notes
                .iter()
                .any(|l| l == "Umwandeln in JPEG, höchstens 2048 px, Qualität 85")
        );
        let en = plan_steps(std::slice::from_ref(&op), Lang::En);
        assert_eq!(en[0].title, "Export 200 photos (Album 'Belege')");
    }

    #[test]
    fn language_parsing() {
        assert_eq!(Lang::parse("de_DE.UTF-8"), Some(Lang::De));
        assert_eq!(Lang::parse("en-US"), Some(Lang::En));
        assert_eq!(Lang::parse("fr_FR"), None);
        assert_eq!(Lang::parse("C"), None);
    }
}
