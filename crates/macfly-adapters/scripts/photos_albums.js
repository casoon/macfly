(function () {
  const Photos = Application("Photos");
  const albums = Photos.albums;
  const count = albums.length;
  if (count === 0) {
    return JSON.stringify({ albums: [] });
  }
  const ids = albums.id();
  const names = albums.name();
  const out = [];
  for (let i = 0; i < count; i++) {
    let n = 0;
    try {
      n = albums[i].mediaItems.length;
    } catch (_) {}
    out.push({ id: String(ids[i]), name: String(names[i] || ""), count: n });
  }
  out.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
  return JSON.stringify({ albums: out });
})();
