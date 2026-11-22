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

function buildFolderPaths(folders) {
  const meta = {};
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
  }
  const paths = {};
  for (const id of Object.keys(meta)) {
    const parts = [];
    const seen = new Set();
    let cur = meta[id];
    while (cur && !seen.has(cur.id)) {
      seen.add(cur.id);
      parts.unshift(cur.name);
      cur = cur.parentId && meta[cur.parentId] ? meta[cur.parentId] : null;
    }
    paths[id] = parts.join("/");
  }
  return paths;
}

(function () {
  const input = readInput();
  const accountFilter = input.account ? String(input.account).toLowerCase() : null;
  const folderFilter = input.folder ? String(input.folder).toLowerCase() : null;
  const query = input.query ? String(input.query).toLowerCase() : null;
  const denyFolders = (input.deny_folders || []).map((s) => String(s).toLowerCase());
  const allowFolders = (input.allow_folders || []).map((s) => String(s).toLowerCase());
  const allowAccounts = (input.allow_accounts || []).map((s) => String(s).toLowerCase());
  const skipProtected = input.skip_password_protected !== false;
  const limit = Number(input.limit || 0);

  const Notes = Application("Notes");
  const out = [];
  const accounts = Notes.accounts();

  for (let i = 0; i < accounts.length; i++) {
    const acc = accounts[i];
    const accName = String(acc.name());
    const accLower = accName.toLowerCase();

    if (allowAccounts.length > 0 && !allowAccounts.includes(accLower)) continue;
    if (accountFilter && accLower !== accountFilter) continue;

    const folders = acc.folders();
    const paths = buildFolderPaths(folders);

    for (let j = 0; j < folders.length; j++) {
      const f = folders[j];
      const fId = String(f.id());
      const fName = String(f.name());
      const fPath = paths[fId] || fName;
      const fLower = fName.toLowerCase();
      const fPathLower = fPath.toLowerCase();

      if (denyFolders.includes(fLower) || denyFolders.includes(fPathLower)) continue;
      if (
        allowFolders.length > 0 &&
        !allowFolders.includes(fLower) &&
        !allowFolders.includes(fPathLower)
      ) {
        continue;
      }
      if (folderFilter && fLower !== folderFilter && fPathLower !== folderFilter) continue;

      const count = f.notes.length;
      if (count === 0) continue;

      const ids = f.notes.id();
      const names = f.notes.name();
      const created = f.notes.creationDate();
      const modified = f.notes.modificationDate();
      let protectedFlags = [];
      try {
        protectedFlags = f.notes.passwordProtected();
      } catch (_) {
        protectedFlags = new Array(count).fill(false);
      }

      for (let k = 0; k < count; k++) {
        const isProtected = Boolean(protectedFlags[k]);
        if (skipProtected && isProtected) continue;

        const title = String(names[k] || "(Ohne Titel)");
        if (query && !title.toLowerCase().includes(query)) continue;

        out.push({
          id: String(ids[k]),
          title: title,
          account: accName,
          folder: fName,
          folder_path: fPath,
          created_at: isoOrNull(created[k]),
          modified_at: isoOrNull(modified[k]),
          password_protected: isProtected,
          attachment_count: 0,
        });
      }
    }
  }

  out.sort((a, b) => {
    const am = a.modified_at || "";
    const bm = b.modified_at || "";
    return bm.localeCompare(am);
  });

  const sliced = limit > 0 ? out.slice(0, limit) : out;
  return JSON.stringify({
    total_matched: out.length,
    notes: sliced,
  });
})();
