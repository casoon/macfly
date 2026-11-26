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

(function () {
  const input = readInput();
  const Photos = Application("Photos");
  const item = Photos.mediaItems.byId(String(input.id));
  // Wirft, wenn die ID nicht (mehr) existiert.
  const id = String(item.id());

  const loc = safe(() => item.location(), null);
  const hasLoc =
    Array.isArray(loc) && loc.length >= 2 && Number.isFinite(loc[0]) && Number.isFinite(loc[1]);
  const alt = safe(() => item.altitude(), null);
  const date = safe(() => item.date(), null);
  const kws = safe(() => item.keywords(), []);
  const title = safe(() => item.name(), null);
  const description = safe(() => item.description(), null);
  const meta = {
    id: id,
    filename: String(safe(() => item.filename(), "")),
    title: title ? String(title) : null,
    description: description ? String(description) : null,
    date: date ? new Date(date).toISOString() : null,
    keywords: Array.isArray(kws) ? kws.map(String) : [],
    favorite: !!safe(() => item.favorite(), false),
    width: safe(() => item.width(), null),
    height: safe(() => item.height(), null),
    size_bytes: safe(() => item.size(), null),
    latitude: hasLoc ? loc[0] : null,
    longitude: hasLoc ? loc[1] : null,
    altitude: Number.isFinite(alt) ? alt : null,
  };

  // Photos lädt Originale aus iCloud bei Bedarf selbst nach; der Befehl kehrt erst danach zurück.
  Photos.export([item], {
    to: Path(String(input.staging_dir)),
    usingOriginals: !!input.originals,
  });

  return JSON.stringify({ ok: true, meta: meta });
})();
