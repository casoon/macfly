ObjC.import("Foundation");

function readInput() {
  const data = $.NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile;
  const raw = $.NSString.alloc.initWithDataEncoding(data, $.NSUTF8StringEncoding).js;
  return raw && raw.trim().length > 0 ? JSON.parse(raw) : {};
}

function escapeHtml(text) {
  return String(text)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\n/g, "<br>");
}

function findAccount(Notes, accountName) {
  const accounts = Notes.accounts();
  if (!accountName) return accounts[0];
  const needle = String(accountName).toLowerCase();
  for (let i = 0; i < accounts.length; i++) {
    if (String(accounts[i].name()).toLowerCase() === needle) return accounts[i];
  }
  throw new Error("Notes-Account nicht gefunden: " + accountName);
}

function buildFolderIndex(acc) {
  const folders = acc.folders();
  const meta = {};
  const byId = {};
  for (let j = 0; j < folders.length; j++) {
    const f = folders[j];
    const id = String(f.id());
    const name = String(f.name());
    let parentId = null;
    try {
      const c = f.container();
      if (c && c.id) parentId = String(c.id());
    } catch (_) {}
    meta[id] = { id, name, parentId };
    byId[id] = f;
  }
  const byPath = {};
  const byName = {};
  for (const id of Object.keys(meta)) {
    const parts = [];
    const seen = new Set();
    let cur = meta[id];
    while (cur && !seen.has(cur.id)) {
      seen.add(cur.id);
      parts.unshift(cur.name);
      cur = cur.parentId && meta[cur.parentId] ? meta[cur.parentId] : null;
    }
    const path = parts.join("/");
    byPath[path.toLowerCase()] = byId[id];
    const lowerName = meta[id].name.toLowerCase();
    if (!byName[lowerName]) byName[lowerName] = byId[id];
  }
  return { byId, byPath, byName };
}

function findFolder(acc, spec) {
  const idx = buildFolderIndex(acc);
  const needle = String(spec).toLowerCase();
  if (idx.byPath[needle]) return idx.byPath[needle];
  if (idx.byName[needle]) return idx.byName[needle];
  throw new Error("Ordner nicht gefunden: " + spec);
}

function findFolderById(Notes, folderId) {
  const accounts = Notes.accounts();
  for (let i = 0; i < accounts.length; i++) {
    const folders = accounts[i].folders();
    for (let j = 0; j < folders.length; j++) {
      if (String(folders[j].id()) === String(folderId)) {
        return { folder: folders[j], account: accounts[i] };
      }
    }
  }
  throw new Error("Ordner-ID nicht gefunden: " + folderId);
}

function findNoteById(Notes, noteId) {
  const accounts = Notes.accounts();
  for (let i = 0; i < accounts.length; i++) {
    const folders = accounts[i].folders();
    for (let j = 0; j < folders.length; j++) {
      const f = folders[j];
      if (f.notes.length === 0) continue;
      const ids = f.notes.id();
      const idx = ids.indexOf(noteId);
      if (idx !== -1) return { note: f.notes[idx], folder: f, account: accounts[i] };
    }
  }
  throw new Error("Notiz nicht gefunden: " + noteId);
}

(function () {
  const input = readInput();
  const Notes = Application("Notes");
  const action = String(input.action || "");

  if (action === "notes_create") {
    const acc = findAccount(Notes, input.account);
    const folder = findFolder(acc, input.folder);
    const html = "<h1>" + escapeHtml(input.title) + "</h1><div>" + escapeHtml(input.body) + "</div>";
    const note = Notes.Note({ body: html });
    folder.notes.push(note);
    return JSON.stringify({
      ok: true,
      detail: "Notiz '" + input.title + "' erstellt in '" + folder.name() + "'",
    });
  }

  if (action === "notes_append") {
    const found = findNoteById(Notes, input.note_id);
    const existing = String(found.note.body() || "");
    found.note.body = existing + "<div><br></div><div>" + escapeHtml(input.text) + "</div>";
    return JSON.stringify({
      ok: true,
      detail: "Text an Notiz '" + found.note.name() + "' angehängt",
    });
  }

  if (action === "notes_move") {
    const found = findNoteById(Notes, input.note_id);
    const destAcc = input.destination_account
      ? findAccount(Notes, input.destination_account)
      : found.account;
    const destFolder = findFolder(destAcc, input.destination_folder);
    Notes.move(found.note, { to: destFolder });
    return JSON.stringify({
      ok: true,
      detail: "Notiz '" + found.note.name() + "' verschoben nach '" + input.destination_folder + "'",
    });
  }

  if (action === "notes_move_folder_contents") {
    const src = findFolderById(Notes, input.source_folder_id);
    const destAcc = input.destination_account
      ? findAccount(Notes, input.destination_account)
      : src.account;
    const destFolder = findFolder(destAcc, input.destination_folder_path);

    if (String(src.folder.id()) === String(destFolder.id())) {
      return JSON.stringify({
        ok: true,
        detail: "Quell- und Zielordner identisch ('" + input.source_folder_path + "') — übersprungen",
      });
    }

    const total = src.folder.notes.length;
    if (total === 0) {
      return JSON.stringify({
        ok: true,
        detail: "0 Notizen in '" + input.source_folder_path + "' (bereits leer)",
      });
    }

    let moved = 0;
    for (let k = total - 1; k >= 0; k--) {
      Notes.move(src.folder.notes[k], { to: destFolder });
      moved++;
    }
    const remaining = src.folder.notes.length;
    return JSON.stringify({
      ok: true,
      detail:
        moved +
        " Notizen verschoben: '" +
        input.source_folder_path +
        "' → '" +
        input.destination_folder_path +
        "' (verbleibend: " +
        remaining +
        ")",
    });
  }

  if (action === "notes_create_folder") {
    const acc = findAccount(Notes, input.account);
    const idx = buildFolderIndex(acc);
    const fullPath = input.parent_folder
      ? String(input.parent_folder) + "/" + String(input.name)
      : String(input.name);
    if (idx.byPath[fullPath.toLowerCase()]) {
      return JSON.stringify({
        ok: true,
        detail: "Ordner '" + fullPath + "' existiert bereits",
      });
    }
    const newFolder = Notes.Folder({ name: String(input.name) });
    if (input.parent_folder) {
      const parent = findFolder(acc, input.parent_folder);
      parent.folders.push(newFolder);
    } else {
      acc.folders.push(newFolder);
    }
    return JSON.stringify({
      ok: true,
      detail: "Ordner '" + fullPath + "' erstellt",
    });
  }

  if (action === "notes_delete_empty_folder") {
    const found = findFolderById(Notes, input.folder_id);
    const f = found.folder;
    const targetId = String(f.id());
    if (f.notes.length > 0) {
      throw new Error(
        "Ordner '" + f.name() + "' ist nicht leer (" + f.notes.length + " Notizen) — Abbruch!"
      );
    }
    // Prüfe aktive Unterordner direkt über container().id() statt gecachtem f.folders.length
    const allFolders = found.account.folders();
    let activeChildren = 0;
    for (let j = 0; j < allFolders.length; j++) {
      try {
        const c = allFolders[j].container();
        if (c && c.id && String(c.id()) === targetId) activeChildren++;
      } catch (_) {}
    }
    if (activeChildren > 0) {
      throw new Error(
        "Ordner '" + f.name() + "' hat noch " + activeChildren + " aktive Unterordner — Abbruch!"
      );
    }
    Notes.delete(f);
    return JSON.stringify({
      ok: true,
      detail: "Leeren Ordner '" + input.folder_name + "' gelöscht",
    });
  }

  throw new Error("Unbekannte Notes-Aktion: " + action);
})();
