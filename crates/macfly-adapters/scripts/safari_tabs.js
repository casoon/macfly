ObjC.import("Foundation");

function readInput() {
  const data = $.NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile;
  const raw = $.NSString.alloc.initWithDataEncoding(data, $.NSUTF8StringEncoding).js;
  return raw && raw.trim().length > 0 ? JSON.parse(raw) : {};
}

(function () {
  const input = readInput();
  const query = input.query ? String(input.query).toLowerCase() : null;

  const Safari = Application("Safari");
  if (!Safari.running()) {
    return JSON.stringify({ total_tabs: 0, tabs: [] });
  }

  const wins = Safari.windows();
  const out = [];

  for (let w = 0; w < wins.length; w++) {
    let tabs = [];
    try {
      tabs = wins[w].tabs();
    } catch (_) {
      tabs = [];
    }
    for (let t = 0; t < tabs.length; t++) {
      let title = "";
      let url = "";
      try {
        title = String(tabs[t].name() || "");
      } catch (_) {}
      try {
        url = String(tabs[t].url() || "");
      } catch (_) {}
      if (!url) continue;

      if (
        query &&
        !title.toLowerCase().includes(query) &&
        !url.toLowerCase().includes(query)
      ) {
        continue;
      }

      out.push({
        window_index: w + 1,
        tab_index: t + 1,
        title: title || url,
        url: url,
      });
    }
  }

  return JSON.stringify({
    total_tabs: out.length,
    tabs: out,
  });
})();
