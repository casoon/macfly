ObjC.import("Foundation");

function readInput() {
  const data = $.NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile;
  const raw = $.NSString.alloc.initWithDataEncoding(data, $.NSUTF8StringEncoding).js;
  return raw && raw.trim().length > 0 ? JSON.parse(raw) : {};
}

(function () {
  const input = readInput();
  const Music = Application("Music");
  const out = [];
  const pls = Music.userPlaylists();
  for (let i = 0; i < pls.length; i++) {
    const p = pls[i];
    let kind = "playlist";
    try {
      if (p.specialKind() === "folder") kind = "folder";
      else if (p.smart()) kind = "smart";
    } catch (_) {}
    let count = 0;
    try {
      count = p.tracks.length;
    } catch (_) {}
    out.push({ name: String(p.name()), kind: kind, track_count: Number(count) });
  }
  const limit = Number(input.limit || 0);
  return JSON.stringify({
    total_matched: out.length,
    playlists: limit > 0 ? out.slice(0, limit) : out,
  });
})();
