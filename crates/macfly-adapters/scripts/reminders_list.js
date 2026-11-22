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

    // Batch property fetch on open reminders (3 Apple Events per list instead of N*3)
    const open = lst.reminders.whose({ completed: false });
    let names = [];
    try {
      names = open.name() || [];
    } catch (_) {
      names = [];
    }
    if (names.length === 0) continue;

    let dues = [];
    let priorities = [];
    try {
      dues = open.dueDate() || [];
    } catch (_) {
      dues = new Array(names.length).fill(null);
    }
    try {
      priorities = open.priority() || [];
    } catch (_) {
      priorities = new Array(names.length).fill(0);
    }

    for (let j = 0; j < names.length; j++) {
      const title = String(names[j] || "(Ohne Titel)");
      let due = null;
      let overdue = false;
      if (dues[j]) {
        due = isoOrNull(dues[j]);
        if (due) overdue = new Date(dues[j]).getTime() < nowMs;
      }
      if (overdueOnly && !overdue) continue;

      out.push({
        list: listName,
        title: title,
        due_at: due,
        overdue: overdue,
        priority: Number(priorities[j] || 0),
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
