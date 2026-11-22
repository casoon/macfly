ObjC.import("Foundation");

function readInput() {
  const data = $.NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile;
  const raw = $.NSString.alloc.initWithDataEncoding(data, $.NSUTF8StringEncoding).js;
  return raw && raw.trim().length > 0 ? JSON.parse(raw) : {};
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
      if (c && c.id) {
        parentId = String(c.id());
      }
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
  readInput();
  const Notes = Application("Notes");
  const accountsOut = [];
  const foldersOut = [];

  const accounts = Notes.accounts();
  for (let i = 0; i < accounts.length; i++) {
    const acc = accounts[i];
    const accId = String(acc.id());
    const accName = String(acc.name());
    const folders = acc.folders();
    const paths = buildFolderPaths(folders);
    let accNoteCount = 0;

    for (let j = 0; j < folders.length; j++) {
      const f = folders[j];
      const fId = String(f.id());
      const fName = String(f.name());
      const fPath = paths[fId] || fName;
      const count = f.notes.length;
      const prefix = fPath + "/";
      let subCount = 0;
      for (const otherId of Object.keys(paths)) {
        if (paths[otherId].startsWith(prefix)) subCount++;
      }
      accNoteCount += count;
      foldersOut.push({
        id: fId,
        name: fName,
        path: fPath,
        account: accName,
        note_count: count,
        subfolder_count: subCount,
      });
    }

    accountsOut.push({
      id: accId,
      name: accName,
      folder_count: folders.length,
      note_count: accNoteCount,
    });
  }

  foldersOut.sort((a, b) => a.path.localeCompare(b.path));

  return JSON.stringify({
    accounts: accountsOut,
    folders: foldersOut,
  });
})();
