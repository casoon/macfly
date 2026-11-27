ObjC.import("Foundation");

function readInput() {
  const data = $.NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile;
  const raw = $.NSString.alloc.initWithDataEncoding(data, $.NSUTF8StringEncoding).js;
  return raw && raw.trim().length > 0 ? JSON.parse(raw) : {};
}

function batch(coll, fn, n, fallback) {
  try {
    const v = coll[fn]();
    if (v && v.length === n) return v;
  } catch (_) {}
  return new Array(n).fill(fallback);
}

function iso(d) {
  if (!d) return null;
  const t = new Date(d);
  return isNaN(t.getTime()) ? null : t.toISOString();
}

(function () {
  const Music = Application("Music");
  const tr = Music.libraryPlaylists[0].tracks;
  const names = tr.name() || [];
  const n = names.length;
  const ids = batch(tr, "persistentID", n, "");
  const artists = batch(tr, "artist", n, "");
  const albums = batch(tr, "album", n, "");
  const genres = batch(tr, "genre", n, "");
  const plays = batch(tr, "playedCount", n, 0);
  const skips = batch(tr, "skippedCount", n, 0);
  const played = batch(tr, "playedDate", n, null);
  const added = batch(tr, "dateAdded", n, null);
  const ratings = batch(tr, "rating", n, 0);
  const durations = batch(tr, "duration", n, 0);
  const now = Date.now();
  const out = [];
  for (let i = 0; i < n; i++) {
    const p = iso(played[i]);
    out.push({
      id: String(ids[i]),
      title: String(names[i]),
      artist: String(artists[i]),
      album: String(albums[i]),
      genre: String(genres[i]),
      played_count: Number(plays[i] || 0),
      skipped_count: Number(skips[i] || 0),
      last_played: p,
      days_since_played: p ? Math.floor((now - new Date(p).getTime()) / 86400000) : null,
      added_at: iso(added[i]),
      rating: Number(ratings[i] || 0),
      duration_secs: Math.round(Number(durations[i] || 0)),
    });
  }
  return JSON.stringify({ tracks: out });
})();
