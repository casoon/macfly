ObjC.import("Foundation");

function readInput() {
  const data = $.NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile;
  const raw = $.NSString.alloc.initWithDataEncoding(data, $.NSUTF8StringEncoding).js;
  return raw && raw.trim().length > 0 ? JSON.parse(raw) : {};
}

function safe(fn, fallback) {
  try {
    const v = fn();
    return v === undefined || v === null ? fallback : v;
  } catch (_) {
    return fallback;
  }
}

function iso(d) {
  try {
    return d ? new Date(d).toISOString() : null;
  } catch (_) {
    return null;
  }
}

(function () {
  const input = readInput();
  const Photos = Application("Photos");
  const keyword = input.keyword ? String(input.keyword).toLowerCase() : null;
  const since = input.since ? new Date(String(input.since)) : null;
  const limit = Number(input.limit || 0);

  let items = Photos.mediaItems;
  if (input.album) {
    const needle = String(input.album).toLowerCase();
    const albums = Photos.albums();
    let found = null;
    for (let i = 0; i < albums.length; i++) {
      if (String(albums[i].name()).toLowerCase() === needle) {
        found = albums[i];
        break;
      }
    }
    if (!found) throw new Error("Album nicht gefunden: " + input.album);
    items = found.mediaItems;
  }

  const count = items.length;
  if (count === 0) {
    return JSON.stringify({ total_matched: 0, items: [] });
  }

  // Günstige Felder stapelweise für alle Einträge, schwere Felder nur für die Treffer.
  const ids = items.id();
  const dates = safe(() => items.date(), new Array(count).fill(null));
  const filenames = safe(() => items.filename(), new Array(count).fill(""));
  const titles = safe(() => items.name(), new Array(count).fill(null));
  const descriptions = safe(() => items.description(), new Array(count).fill(null));
  const keywordsAll = keyword
    ? safe(() => items.keywords(), new Array(count).fill(null))
    : null;

  const hits = [];
  for (let i = 0; i < count; i++) {
    const d = dates[i] ? new Date(dates[i]) : null;
    if (since && (!d || d < since)) continue;
    if (keyword) {
      const kws = Array.isArray(keywordsAll[i]) ? keywordsAll[i] : [];
      if (!kws.some((k) => String(k).toLowerCase().includes(keyword))) continue;
    }
    hits.push({ i: i, t: d ? d.getTime() : 0 });
  }
  hits.sort((a, b) => b.t - a.t);
  const total = hits.length;
  const chosen = limit > 0 ? hits.slice(0, limit) : hits;

  const out = chosen.map((h) => {
    const i = h.i;
    const item = items[i];
    const loc = safe(() => item.location(), null);
    const hasLoc =
      Array.isArray(loc) && loc.length >= 2 && Number.isFinite(loc[0]) && Number.isFinite(loc[1]);
    const alt = safe(() => item.altitude(), null);
    const kws = safe(() => item.keywords(), []);
    return {
      id: String(ids[i]),
      filename: String(filenames[i] || ""),
      title: titles[i] ? String(titles[i]) : null,
      description: descriptions[i] ? String(descriptions[i]) : null,
      date: iso(dates[i]),
      keywords: Array.isArray(kws) ? kws.map(String) : [],
      favorite: !!safe(() => item.favorite(), false),
      width: safe(() => item.width(), null),
      height: safe(() => item.height(), null),
      size_bytes: safe(() => item.size(), null),
      latitude: hasLoc ? loc[0] : null,
      longitude: hasLoc ? loc[1] : null,
      altitude: Number.isFinite(alt) ? alt : null,
    };
  });

  return JSON.stringify({ total_matched: total, items: out });
})();
