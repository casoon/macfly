# Bewertung, Schwachstellenanalyse, Sicherheit & USP-Strategie

> **Bezug:** Kritische Prüfung von [`konzept.txt`](../konzept.txt)
> **Stand:** November 2026

---

## 1. Gesamtbewertung des Ausgangskonzepts

Das Ausgangskonzept trifft eine **sehr gute strategische Grundintuition**: Der Mehrwert liegt nicht in einem weiteren AppleScript-Wrapper, sondern in einer **kontrollierbaren, typisierten Ausführungsschicht zwischen KI-Agenten und persönlichen macOS-Daten**.

**Was bereits überzeugt:**
- **CLI-first & modellunabhängig:** Kein Lock-in an einen bestimmten KI-Client; funktioniert mit Claude Code, Codex, Gemini CLI, Shell-Skripten und optionalem MCP.
- **Plan → Review → Apply mit Revision-Checks:** Deutlich robuster als simples `--confirm`.
- **Ehrliche Grenzen:** Keine Illusion einer echten Transaktion (`reversible` / `best_effort` / `irreversible`), realistische Einschätzung von Apple Mail (lokaler Bestand, instabile IDs, EWS→Graph-Übergang).
- **Wahrnehmung von Indirect Prompt Injection:** E-Mails explizit als untrusted data behandeln.

**Kurzurteil:** Als erste Ideenskizze überdurchschnittlich gut durchdacht. Als **produktives Sicherheits- und Open-Source-Konzept** hat es jedoch **fünf kritische Lücken**, die gelöst werden müssen, damit es gegenüber bestehenden Tools (`pippin`, `apple-tools`, `boutquin/apple-bridge`) tatsächlich *Best Practice* wird.

---

## 2. Konkrete Schwachstellen im bestehenden Konzept

### Schwachstelle 1: Das TCC- & Self-Approval-Paradoxon (Architektur-Bruch)
Das Konzept schlägt eine CLI vor, die bei Bedarf einen lokalen Freigabeprozess startet, und erwähnt Touch ID (`LAContext`). Gleichzeitig räumt es ein, dass ein Agent mit Shell-Rechten die Bridge umgehen kann.

- **Das eigentliche macOS-Problem (TCC Responsible Process Chain):** Wenn ein KI-Agent (z. B. im Terminal oder einer IDE) `bridge` als Kindprozess startet, ordnet macOS TCC die Apple-Events- und Full-Disk-Access-Rechte dem **verantwortlichen Elternprozess** (Terminal/IDE) zu.
- **Folge:** Entweder besitzt das Terminal/der Agent selbst diese Rechte — dann kann eine Prompt Injection einfach direkt `osascript` aufrufen und die Bridge komplett umgehen. Oder das Terminal besitzt sie *nicht* — dann funktioniert eine normale Kindprozess-CLI gar nicht!
- **Lösung:** Echte **Privilege Separation** über einen separat signierten lokalen Broker (LaunchAgent / Helper-`.app`-Bundle mit eigener TCC-Identität), mit dem die Agent-CLI nur über einen eingeschränkten Unix-Domain-Socket oder XPC spricht.

### Schwachstelle 2: Read-Side-Sicherheit (Datenabfluss) fehlt fast vollständig
90 % des Sicherheitskonzepts betreffen *Schreiboperationen*. Im Alltag mit Cloud-LLMs ist jedoch **unbeabsichtigte Datenexfiltration beim Lesen** das wahrscheinlichere und folgenreichere Risiko:
- Eine einzige `notes search` oder `mail search` kann Passwörter, 2FA-Codes, IBANs, Steuerdaten, Arztbriefe oder vertrauliche Kundendaten ungefiltert an einen externen Modellanbieter übertragen.
- Eine bösartige E-Mail (Indirect Prompt Injection) kann den Agenten anweisen, sensible Mails auszulesen und deren Inhalt über einen anderen Kanal (z. B. Web-Fetch, URL-Parameter, Antwort-Entwurf) abfließen zu lassen.

### Schwachstelle 3: Rust-only vs. macOS-Realität
Das Konzept verkauft „keine Node.js-Laufzeit“ und „Rust“ als großen Vorteil.
- **Realität:** Die stärksten Konkurrenten (`pippin`, `boutquin/apple-bridge`) sind **in Swift** geschrieben — ebenfalls native Binaries ohne Laufzeitumgebung.
- Auf macOS ist Swift für Apple-Frameworks (`EventKit`, `Contacts`, `LocalAuthentication`, `App Intents`, `XPC`, Code Signing / `.app`-Bundling) First-Class-Citizen. Rust-Bindings (`objc2-*`) decken nur Objective-C-APIs ab, keine Swift-only-APIs (wie moderne *App Intents*), und erhöhen den Wartungsaufwand bei macOS-Updates.
- **Richtiger Schnitt:** Rust ist hervorragend für den **Core** (Policy Engine, Plan-Verifikation, Kryptografie, Schema-Validierung, MIME/HTML-Parsing von untrusted Content, PII-Redaction, SQLite-Index, CLI). Für den macOS-Privilegien-Helper (Touch ID GUI, XPC, EventKit, App Intents) ist ein dünner Swift-Helper oder gezieltes `objc2` die pragmatischere Wahl.

### Schwachstelle 4: Unterschätzte Datenverlust-Fallen bei Apple Notes & Mail
- **Destruktives Überschreiben in Apple Notes:** Über AppleScript liefert `body` proprietäres HTML. Ein `set body` zerstört stillschweigend eingebettete PDFs, Bilder, Checklisten, Tabellen und interne Notiz-Links (`applenotes://`). Das darf niemals als normales `edit` durchgehen.
- **Token-Explosion in Apple Mail:** Rohe Mails bestehen oft zu 70–90 % aus HTML-Tabellen, Tracking-Pixeln, Base64-Signaturen und verschachtelten Zitat-Ketten. Ohne dedizierte Bereinigung und Thread-Deduplizierung verbraucht die Bridge unnötig Kontextfenster und verschlechtert die KI-Qualität.
- **Argv-Leakage bei `osascript`:** Werden Suchbegriffe, Mailtexte oder Notizinhalte als Kommandozeilenargumente an `osascript` übergeben, sind sie kurzzeitig über `ps aux` für andere lokale Prozesse sichtbar. Datenübergabe muss über `stdin` oder anonyme Pipes erfolgen.

### Schwachstelle 5: Over-Engineering im Start-Layout & Namenskollision
- **12 Cargo-Crates ab Tag 1** erzeugen massiven Trait- und Wartungs-Overhead vor dem ersten funktionierenden End-to-End-Lauf.
- **Namenskollision:** `boutquin/apple-bridge` belegt den Namen `apple-bridge` bereits öffentlich auf GitHub. Für Open Source, Homebrew und Crates.io braucht das Projekt früh einen eigenständigen Namen.

---

## 3. Sicherheitsanalyse (Threat Model)

Ein seriöses Sicherheitskonzept muss explizit definieren, **gegen wen** es schützt und **wo** die Systemgrenze liegt.

| Bedrohung | Angriffspfad | Schutzmaßnahme im Zielkonzept |
|---|---|---|
| **T1: Indirect Prompt Injection (Write)** | Bösartige E-Mail weist Agent an, Mails weiterzuleiten, Notizen zu löschen oder Daten zu manipulieren | Agent kann nur *Pläne vorschlagen*; Broker erzwingt Policy + Out-of-Band-Freigabe (Touch ID / nativer Dialog) für kritische Aktionen |
| **T2: Self-Approval / Bypass** | Agent bestätigt Terminal-Prompt selbst oder ruft direkt `osascript` auf | **Privilege Separation:** Agent-Prozess hat *keine* TCC-Rechte; nur der signierte Broker-Prozess hält TCC-Rechte und zeigt native OS-Freigabe außerhalb des Agent-Terminals |
| **T3: Plan-Manipulation (TOCTOU)** | Agent ändert Plandatei zwischen Review und Apply, oder Daten ändern sich dazwischen | Plan wird im Broker gespeichert (nicht im Agent-Workspace), kryptografisch an SHA-256-Hash + Nonce + TTL gebunden; Revision-Check unmittelbar vor Ausführung |
| **T4: Read-Side Data Exfiltration** | Agent liest private Ordner/Konten oder sensible Secrets und sendet sie an ein Cloud-LLM | **Read-Firewall (DLP):** Account-/Ordner-Allowlists pro Profil, automatische PII-/Secret-Redaction (IBAN, 2FA, API-Keys, Passwörter), Rate- & Volumenlimits |
| **T5: Command / Script Injection** | Sonderzeichen oder Anführungszeichen in Mail-Betreffs brechen aus AppleScript aus | Strikt parametrisierte Skripte; keine String-Konkatenation; Payload-Übergabe als JSON über `stdin` (kein `argv`-Leak) |
| **T6: Sensible Daten im Audit-Journal** | Undo-Backup speichert gelöschte Mails/Notizen ungeschützt auf der Platte | Speicherort im geschützten Broker-Verzeichnis (`0700`/`0600`), optionale Keychain-Verschlüsselung, konfigurierbare Retention & Redaction |
| **T7: Destruktiver Datenverlust** | Überschreiben einer Rich-Note zerstört Anhänge/Checklisten | **Fidelity-Vertrag:** Erkennung von Anhängen/Rich-Content → Blockierung von `replace`, nur `append` oder explizites Override erlaubt |

**Klare Systemgrenze (ehrlich dokumentieren):**
- Gegen einen lokalen Angreifer mit `root`-Rechten oder vollem Zugriff auf den Benutzer-Account (der den Broker patchen oder eigene TCC-Rechte besitzt) schützt kein User-Space-Tool.
- Der Schutz greift voll, wenn der KI-Agent in einem **eingeschränkten Profil/Sandbox ohne eigene Automation-/FDA-Rechte** läuft und Apple-Daten ausschließlich über den Broker erreicht.

---

## 4. Best-Practice-Check gegenüber Mitkonkurrenten

| Dimension | Typische Konkurrenz (`pippin`, `apple-tools`, `boutquin/apple-bridge`, JS-MCPs) | Best Practice 2026 | Bewertung des Ausgangskonzepts |
|---|---|---|---|
| **Zugriffs-Isolation** | Läuft im selben Prozess/Kontext wie der Agent (Agent erbt/teilt TCC) | Getrennter Broker-Prozess (eigene TCC-Identität) + dünne Agent-CLI | Noch nicht erreicht (erst durch Broker-Architektur) |
| **Approval-UX** | `--confirm`, Token im Terminal oder JSON-Flag | Out-of-Band-Freigabe (nativer macOS-Dialog + Touch ID) gebunden an Plan-Hash | Als Idee (Stufe B) vorhanden, muss Kernbestandteil werden |
| **Read-Sicherheit (DLP)** | Meist nur Account-Filter oder gar keine Redaction | Profil-Scopes + automatische PII-/Secret-Redaction + Untrusted-Framing | Nur kurz erwähnt → **große Differenzierungschance** |
| **Interface** | Entweder nur CLI oder nur MCP | **Dual-Interface aus einer Binary:** CLI (JSON/Agent-Format) + `mcp` (`stdio`) | CLI-first ist gut; `mcp` sollte als dünner Wrapper (P1 statt P2) mitgedacht werden |
| **Apple-Backends** | Oft entweder reines AppleScript (langsam) oder harter SQLite-Zugriff (fragil) | Offizielle APIs zuerst (EventKit/Contacts/App Intents), AppleScript gebatcht, optionaler Read-Only-Index mit Versions-Check | Sehr gut getroffen |
| **Token-Effizienz** | Oft rohes HTML oder komplette Objekte | HTML→Clean-Markdown, Thread-Deduplizierung, Feld-Projektion, Token-Budget | Gut angelegt, muss technisch konkretisiert werden |

---

## 5. Wege für ein echtes Alleinstellungsmerkmal (USP)

„Noch ein macOS-CLI/MCP in Rust“ ist **kein** USP. Ein echtes, schwer kopierbares Alleinstellungsmerkmal entsteht durch die Positionierung als **„Security-First Data Firewall & Execution Broker zwischen KI-Agenten und macOS“**:

### Die 5 tragenden USP-Säulen

1. **Echte TCC-Privilege-Separation („Zero-TCC Agent“)**
   Kein bekannter Konkurrent trennt konsequent zwischen dem *untrusted Agent-Client* (ohne macOS-Automation-Rechte) und einem *signierten lokalen Broker* (hält TCC-Rechte). Wer KI-Agenten ernsthaft auf private/geschäftliche Mails loslässt, braucht genau diese Grenze.

2. **Kryptografisch gebundene Out-of-Band-Freigabe (Touch ID / Native Sheet)**
   Ein Plan-Review, das nicht im selben Terminal wie der Agent stattfindet, sondern als natives macOS-Panel des Brokers mit Diff, Risiko-Badge, Reversibilitäts-Angabe und Touch-ID-Bestätigung. Der Agent kann sich physikalisch nicht selbst bestätigen.

3. **Read-Side Data Firewall (DLP & Profil-Scopes)**
   Feingranulare, deklarative Policies (TOML/YAML) pro Agenten-Profil:
   - *„Profil `codex-work` darf nur Account `Firma`, Ordner `INBOX`, max. 20 Mails/Stunde lesen, keine Anhänge, und alle IBANs/2FA-Codes/API-Keys werden automatisch redaktiert.“*
   - Das löst das größte praktische Hemmnis vieler Nutzer, KI überhaupt an lokale Mails und Notizen zu lassen.

4. **Fidelity-Garantien (Schutz vor stillem Datenverlust)**
   Statt blindem CRUD prüft die Bridge die Objekt-Treue (`lossless` vs. `rich_content_protected`). Notizen mit Anhängen/Checklisten werden vor zerstörerischem Überschreiben geschützt; Mail-IDs werden stabil über `(Account, RFC-Message-ID, Content-Hash)` aufgelöst.

5. **Token-budgetierte Context Packs mit Untrusted-Framing**
   Ein Aufruf (`context daily-brief --budget 4000`) liefert bereinigte, deduplizierte, PII-gefilterte und klar als `untrusted_external_data` markierte Daten aus Mail, Kalender und Erinnerungen — sofort nutzbar für jedes Modell ohne 15 einzelne Tool-Aufrufe.

---

## 6. Empfehlung für das weitere Vorgehen

1. **Namen festlegen**, der nicht mit `boutquin/apple-bridge` kollidiert (z. B. `kestrel-bridge` oder ein eigenständiger Markenname wie `macgate`, `aegis-mac`, `vellum-mac`).
2. **Architektur in zwei Schichten schneiden** (siehe [`02-konzept-ausarbeitung.md`](./02-konzept-ausarbeitung.md)):
   - **Phase 1 (Schneller Praxis-MVP):** Rust-Core + CLI + Mail/Notes Read-Only + DLP-Redaction + Plan/Diff-Engine (noch lokal im selben Prozess, aber architektonisch strikt über ein Client/Broker-Protokoll getrennt).
   - **Phase 2 (Echte Sicherheitsgrenze):** Auslagerung des Ausführungs- und Approval-Teils in den signierten lokalen Broker (Unix-Socket/XPC + Touch ID).
3. **Praxistest sofort mit Mail + Notes + DLP:** Reales Postfach → bereinigtes `inbox-triage` Context Pack → KI erstellt Änderungsplan → Prüfung & kontrollierte Erstellung einer Zusammenfassungs-Notiz.
