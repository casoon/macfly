ObjC.import("Foundation");

function readInput() {
  const data = $.NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile;
  const raw = $.NSString.alloc.initWithDataEncoding(data, $.NSUTF8StringEncoding).js;
  return raw && raw.trim().length > 0 ? JSON.parse(raw) : {};
}

(function () {
  const input = readInput();
  const limit = Number(input.limit || 100);
  const Music = Application("Music");
  let source;
  if (input.playlist) {
    const found = Music.userPlaylists.whose({ name: String(input.playlist) })();
    if (found.length === 0) throw new Error("Playlist nicht gefunden: " + input.playlist);
    source = found[0];
  } else {
    source = Music.libraryPlaylists[0];
  }
  const tracks = source.tracks;
  const total = tracks.length;
  const n = limit > 0 ? Math.min(limit, total) : total;
  const out = [];
  for (let i = 0; i < n; i++) {
    const t = tracks[i];
    out.push({
      id: String(t.persistentID()),
      title: String(t.name()),
      artist: String(t.artist()),
      album: String(t.album()),
      genre: String(t.genre()),
      year: Number(t.year() || 0),
    });
  }
  return JSON.stringify({ total_matched: total, tracks: out });
})();
