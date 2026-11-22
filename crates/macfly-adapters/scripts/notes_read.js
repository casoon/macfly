ObjC.import("Foundation");

function readInput() {
  const data = $.NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile;
  const raw = $.NSString.alloc.initWithDataEncoding(data, $.NSUTF8StringEncoding).js;
  return raw && raw.trim().length > 0 ? JSON.parse(raw) : {};
}

function isoOrNull(val) {
  if (!val) return null;
  try {
    const d = new Date(val);
    if (isNaN(d.getTime())) return null;
    return d.toISOString();
  } catch (_) {
    return null;
  }
}

(function () {
  const input = readInput();
  const targetId = String(input.id || "");
  if (!targetId) {
    return JSON.stringify({ found: false });
  }

  const Notes = Application("Notes");
  const accounts = Notes.accounts();

  for (let i = 0; i < accounts.length; i++) {
    const acc = accounts[i];
    const accName = String(acc.name());
    const folders = acc.folders();

    for (let j = 0; j < folders.length; j++) {
      const f = folders[j];
      const fName = String(f.name());
      if (f.notes.length === 0) continue;

      const ids = f.notes.id();
      const idx = ids.indexOf(targetId);
      if (idx === -1) continue;

      const note = f.notes[idx];
      let isProtected = false;
      try {
        isProtected = Boolean(note.passwordProtected());
      } catch (_) {}

      let bodyHtml = "";
      let attachmentCount = 0;
      if (!isProtected) {
        try {
          bodyHtml = String(note.body() || "");
        } catch (_) {
          bodyHtml = "";
        }
        try {
          attachmentCount = note.attachments.length;
        } catch (_) {
          attachmentCount = 0;
        }
      }

      return JSON.stringify({
        found: true,
        id: String(note.id()),
        title: String(note.name() || "(Ohne Titel)"),
        account: accName,
        folder: fName,
        created_at: isoOrNull(note.creationDate()),
        modified_at: isoOrNull(note.modificationDate()),
        password_protected: isProtected,
        attachment_count: attachmentCount,
        body_html: bodyHtml,
      });
    }
  }

  return JSON.stringify({ found: false });
})();
