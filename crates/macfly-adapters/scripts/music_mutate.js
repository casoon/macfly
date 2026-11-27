ObjC.import("Foundation");

function readInput() {
  const data = $.NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile;
  const raw = $.NSString.alloc.initWithDataEncoding(data, $.NSUTF8StringEncoding).js;
  return raw && raw.trim().length > 0 ? JSON.parse(raw) : {};
}

(function () {
  const input = readInput();
  const action = String(input.action || "");
  const Music = Application("Music");

  if (action === "music_playlist_add") {
    const name = String(input.playlist || "");
    let created = false;
    let found = Music.userPlaylists.whose({ name: name })();
    let target;
    if (found.length === 0) {
      target = Music.make({ new: "playlist", withProperties: { name: name } });
      created = true;
    } else {
      target = found[0];
    }
    const lib = Music.libraryPlaylists[0];
    const ids = input.track_ids || [];
    let added = 0;
    const missing = [];
    for (let i = 0; i < ids.length; i++) {
      const hits = lib.tracks.whose({ persistentID: String(ids[i]) })();
      if (hits.length === 0) {
        missing.push(String(ids[i]));
        continue;
      }
      Music.duplicate(hits[0], { to: target });
      added++;
    }
    let detail =
      (created ? "Playlist '" + name + "' angelegt, " : "Playlist '" + name + "': ") +
      added + " Titel hinzugefügt";
    if (missing.length > 0) detail += " (" + missing.length + " ID(s) nicht in der Mediathek)";
    return JSON.stringify({ ok: true, detail: detail });
  }

  throw new Error("Unbekannte Music-Aktion: " + action);
})();
