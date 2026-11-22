use std::io::{self, BufRead, Write};

use macfly_adapters::calendar::{CalendarAdapter, RemindersAdapter};
use macfly_adapters::contacts::ContactsAdapter;
use macfly_adapters::mail::{ListMailQuery, MailAdapter};
use macfly_adapters::notes::{ListNotesQuery, NotesAdapter};
use macfly_adapters::safari::SafariAdapter;
use macfly_adapters::shortcuts::ShortcutsAdapter;
use macfly_core::config::{Config, ProfileConfig};
use macfly_core::error::MacflyError;
use macfly_core::plan::{PlanProposal, PlanStore};
use serde_json::{Value, json};

/// Dünner MCP-Server über `stdio` (JSON-RPC 2.0).
/// Sicherheitsdesign: Exponiert NUR Read-Tools sowie `plan_create` / `plan_inspect`.
/// `plan_approve` und `plan_apply` sind absichtlich NICHT über MCP erreichbar,
/// damit kein KI-Agent seine eigenen Änderungspläne freigeben oder ausführen kann.
pub fn run_mcp_server(
    cfg: &Config,
    profile_name: &str,
    profile: &ProfileConfig,
) -> Result<(), MacflyError> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {e}") }
                });
                writeln!(stdout, "{err_resp}")?;
                stdout.flush()?;
                continue;
            }
        };

        let id = req.get("id").cloned();
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or_else(|| json!({}));

        // Benachrichtigungen (ohne id) erwarten keine Antwort
        if id.is_none() {
            continue;
        }

        let response = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "macfly",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            }),
            "ping" => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
            "tools/list" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "tools": tool_definitions() }
            }),
            "tools/call" => {
                let tool_name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                match dispatch_tool(cfg, profile_name, profile, tool_name, &args) {
                    Ok(payload) => json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{
                                "type": "text",
                                "text": serde_json::to_string_pretty(&payload)?
                            }],
                            "isError": false
                        }
                    }),
                    Err(err) => json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{
                                "type": "text",
                                "text": serde_json::to_string_pretty(&err.to_envelope())?
                            }],
                            "isError": true
                        }
                    }),
                }
            }
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {method}") }
            }),
        };

        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }

    Ok(())
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "notes_overview",
            "description": "Übersicht aller Apple Notes Accounts und Ordner (inkl. Hierarchiepfad und Notizanzahl).",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "notes_audit",
            "description": "Aufräum-Audit für Apple Notes: leere Ordner, 1-Notiz-Ordner, doppelte Ordnernamen, doppelte Titel und älteste Notizen.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "notes_list",
            "description": "Listet oder sucht Apple Notes Metadaten (gefiltert durch Profil-Policy).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account": { "type": "string" },
                    "folder": { "type": "string" },
                    "query": { "type": "string" },
                    "limit": { "type": "integer", "default": 50 }
                }
            }
        },
        {
            "name": "notes_read",
            "description": "Liest eine einzelne Apple Note inkl. DLP-Redaction, Fidelity-Info und Revisions-Hash.",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }
        },
        {
            "name": "mail_overview",
            "description": "Übersicht über Apple Mail Konten, Postfächer (inkl. Ungelesen-Zähler) und Signaturen.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "mail_list",
            "description": "Listet oder sucht E-Mails in Apple Mail (Default: INBOX).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account": { "type": "string" },
                    "mailbox": { "type": "string", "default": "INBOX" },
                    "unread_only": { "type": "boolean", "default": false },
                    "query": { "type": "string" },
                    "limit": { "type": "integer", "default": 25 }
                }
            }
        },
        {
            "name": "mail_read",
            "description": "Liest eine E-Mail per local_id (DLP-redaktiert und als untrusted_external markiert).",
            "inputSchema": {
                "type": "object",
                "required": ["local_id"],
                "properties": {
                    "local_id": { "type": "string" },
                    "account": { "type": "string" },
                    "mailbox": { "type": "string" }
                }
            }
        },
        {
            "name": "calendar_events",
            "description": "Liest kommende Termine der nächsten N Tage aus Apple Calendar.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "days": { "type": "integer", "default": 7 },
                    "calendar": { "type": "string" },
                    "limit": { "type": "integer", "default": 50 }
                }
            }
        },
        {
            "name": "reminders_list",
            "description": "Liest offene Erinnerungen (optional nur überfällige) aus Apple Reminders.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list": { "type": "string" },
                    "overdue_only": { "type": "boolean", "default": false },
                    "limit": { "type": "integer", "default": 50 }
                }
            }
        },
        {
            "name": "shortcuts_list",
            "description": "Listet verfügbare macOS-Kurzbefehle inkl. Allowlist-Status (`allowed_by_policy`). Ausführung erfolgt nur über `plan_create` (`shortcuts_run`) + menschliche Freigabe.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "contacts_search",
            "description": "Sucht oder listet Kontakte aus Apple Contacts (Name, Firma, E-Mails, Telefonnummern).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "limit": { "type": "integer", "default": 50 }
                }
            }
        },
        {
            "name": "contacts_audit",
            "description": "Prüft Apple Contacts auf doppelte Namen, doppelte E-Mail-Adressen und Kontakte ohne E-Mail/Telefon.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "safari_tabs",
            "description": "Listet alle aktuell offenen Safari-Tabs (Titel & URL), optional gefiltert nach Suchbegriff.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                }
            }
        },
        {
            "name": "plan_create",
            "description": "Erstellt einen geprüften Änderungsplan (z. B. notes_move, notes_create, notes_delete_empty_folder, mail_create_draft). Führt NICHT direkt aus — der Nutzer gibt den Plan separat im Terminal frei.",
            "inputSchema": {
                "type": "object",
                "required": ["operations"],
                "properties": {
                    "summary": { "type": "string" },
                    "operations": { "type": "array", "items": { "type": "object" } }
                }
            }
        },
        {
            "name": "plan_inspect",
            "description": "Zeigt Status, Hash und Vorschau eines gespeicherten Änderungsplans.",
            "inputSchema": {
                "type": "object",
                "required": ["plan_id"],
                "properties": { "plan_id": { "type": "string" } }
            }
        }
    ])
}

fn dispatch_tool(
    cfg: &Config,
    profile_name: &str,
    profile: &ProfileConfig,
    name: &str,
    args: &Value,
) -> Result<Value, MacflyError> {
    let notes = NotesAdapter::new(cfg.script_timeout_secs);
    let mail = MailAdapter::new(cfg.script_timeout_secs);

    match name {
        "notes_overview" => Ok(serde_json::to_value(notes.overview(profile_name, profile)?)?),
        "notes_audit" => Ok(serde_json::to_value(notes.audit(profile_name, profile)?)?),
        "notes_list" => {
            let q = ListNotesQuery {
                account: args.get("account").and_then(Value::as_str).map(String::from),
                folder: args.get("folder").and_then(Value::as_str).map(String::from),
                query: args.get("query").and_then(Value::as_str).map(String::from),
                limit: args.get("limit").and_then(Value::as_u64).unwrap_or(50) as usize,
            };
            Ok(serde_json::to_value(notes.list(profile_name, profile, &q)?)?)
        }
        "notes_read" => {
            let id = args
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| MacflyError::PlanError("Parameter `id` fehlt".to_string()))?;
            Ok(serde_json::to_value(notes.read(
                profile_name,
                profile,
                id,
                false,
                false,
            )?)?)
        }
        "mail_overview" => Ok(serde_json::to_value(mail.overview(profile_name, profile)?)?),
        "mail_list" => {
            let q = ListMailQuery {
                account: args.get("account").and_then(Value::as_str).map(String::from),
                mailbox: args.get("mailbox").and_then(Value::as_str).map(String::from),
                unread_only: args
                    .get("unread_only")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                query: args.get("query").and_then(Value::as_str).map(String::from),
                limit: args.get("limit").and_then(Value::as_u64).unwrap_or(25) as usize,
                scan_limit: 150,
            };
            Ok(serde_json::to_value(mail.list(profile_name, profile, &q)?)?)
        }
        "mail_read" => {
            let local_id = args
                .get("local_id")
                .and_then(Value::as_str)
                .ok_or_else(|| MacflyError::PlanError("Parameter `local_id` fehlt".to_string()))?;
            let acc = args.get("account").and_then(Value::as_str);
            let mb = args.get("mailbox").and_then(Value::as_str);
            Ok(serde_json::to_value(mail.read(
                profile_name,
                profile,
                local_id,
                acc,
                mb,
                false,
            )?)?)
        }
        "calendar_events" => {
            let cal = CalendarAdapter::new(cfg.script_timeout_secs);
            let days = args.get("days").and_then(Value::as_u64).unwrap_or(7) as u32;
            let filter = args.get("calendar").and_then(Value::as_str);
            let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(50) as usize;
            Ok(serde_json::to_value(cal.events(days, filter, limit)?)?)
        }
        "reminders_list" => {
            let rem = RemindersAdapter::new(cfg.script_timeout_secs);
            let filter = args.get("list").and_then(Value::as_str);
            let overdue = args
                .get("overdue_only")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(50) as usize;
            Ok(serde_json::to_value(rem.list(filter, overdue, limit)?)?)
        }
        "shortcuts_list" => {
            let sc = ShortcutsAdapter::new(cfg.script_timeout_secs);
            Ok(serde_json::to_value(sc.list(profile_name, profile)?)?)
        }
        "contacts_search" => {
            let ct = ContactsAdapter::new(cfg.script_timeout_secs);
            let query = args.get("query").and_then(Value::as_str);
            let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(50) as usize;
            Ok(serde_json::to_value(ct.list(query, limit)?)?)
        }
        "contacts_audit" => {
            let ct = ContactsAdapter::new(cfg.script_timeout_secs);
            Ok(serde_json::to_value(ct.audit()?)?)
        }
        "safari_tabs" => {
            let sf = SafariAdapter::new(cfg.script_timeout_secs);
            let query = args.get("query").and_then(Value::as_str);
            Ok(serde_json::to_value(sf.tabs(query)?)?)
        }
        "plan_create" => {
            let proposal: PlanProposal = serde_json::from_value(args.clone())?;
            let store = PlanStore::default_store();
            let stored = store.create(profile_name, profile, proposal, 900)?;
            Ok(serde_json::to_value(stored)?)
        }
        "plan_inspect" => {
            let plan_id = args
                .get("plan_id")
                .and_then(Value::as_str)
                .ok_or_else(|| MacflyError::PlanError("Parameter `plan_id` fehlt".to_string()))?;
            let store = PlanStore::default_store();
            let stored = store.load(plan_id)?;
            Ok(serde_json::to_value(stored)?)
        }
        _ => Err(MacflyError::NotFound(format!(
            "Unbekanntes MCP-Tool '{name}'"
        ))),
    }
}
