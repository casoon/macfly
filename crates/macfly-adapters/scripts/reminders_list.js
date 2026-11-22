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

(function () {
  const input = readInput();
  const listFilter = input.list ? String(input.list).toLowerCase() : null;
  const overdueOnly = Boolean(input.overdue_only);
  const limit = Number(input.limit || 50);

  const nowMs = Date.now();
  const Reminders = Application("Reminders");
  const lists = Reminders.lists();
  const out = [];

  for (let i = 0; i < lists.length; i++) {
    const lst = lists[i];
    const listName = String(lst.name());
    if (listFilter && listName.toLowerCase() !== listFilter) continue;

    let items = [];
    try {
      items = lst.reminders.whose({ completed: false })();
    } catch (_) {
      items = [];
    }

    for (let j = 0; j < items.length; j++) {
      const r = items[j];
      let title = "(Ohne Titel)";
      let due = null;
      let overdue = false;
      let priority = 0;
      try { title = String(r.name() || "(Ohne Titel)"); } catch (_) {}
      try {
        const d = r.dueDate();
        if (d) {
          due = isoOrNull(d);
          overdue = new Date(d).getTime() < nowMs;
        }
      } catch (_) {}
      try { priority = Number(r.priority() || 0); } catch (_) {}

      if (overdueOnly && !overdue) continue;

      out.push({
        list: listName,
        title: title,
        due_at: due,
        overdue: overdue,
        priority: priority,
      });
    }
  }

  out.sort((a, b) => {
    if (a.overdue !== b.overdue) return a.overdue ? -1 : 1;
    return (a.due_at || "9999").localeCompare(b.due_at || "9999");
  });

  return JSON.stringify({
    total_matched: out.length,
    reminders: limit > 0 ? out.slice(0, limit) : out,
  });
})();
