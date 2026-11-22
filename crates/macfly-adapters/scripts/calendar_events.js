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
  const days = Number(input.days || 7);
  const calendarFilter = input.calendar ? String(input.calendar).toLowerCase() : null;
  const limit = Number(input.limit || 50);

  const now = new Date();
  const startWindow = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 0, 0, 0);
  const endWindow = new Date(startWindow.getTime() + days * 24 * 60 * 60 * 1000);

  const Calendar = Application("Calendar");
  const cals = Calendar.calendars();
  const out = [];

  for (let i = 0; i < cals.length; i++) {
    const cal = cals[i];
    const calName = String(cal.name());
    if (calendarFilter && calName.toLowerCase() !== calendarFilter) continue;

    let events = [];
    try {
      events = cal.events.whose({
        _and: [
          { startDate: { _greaterThanEquals: startWindow } },
          { startDate: { _lessThanEquals: endWindow } },
        ],
      })();
    } catch (_) {
      events = [];
    }

    for (let j = 0; j < events.length; j++) {
      const ev = events[j];
      let title = "(Ohne Titel)";
      let start = null;
      let end = null;
      let allDay = false;
      let location = null;
      try { title = String(ev.summary() || "(Ohne Titel)"); } catch (_) {}
      try { start = isoOrNull(ev.startDate()); } catch (_) {}
      try { end = isoOrNull(ev.endDate()); } catch (_) {}
      try { allDay = Boolean(ev.alldayEvent()); } catch (_) {}
      try {
        const loc = ev.location();
        if (loc) location = String(loc);
      } catch (_) {}

      out.push({
        calendar: calName,
        title: title,
        start_at: start,
        end_at: end,
        all_day: allDay,
        location: location,
      });
    }
  }

  out.sort((a, b) => (a.start_at || "").localeCompare(b.start_at || ""));

  return JSON.stringify({
    total_matched: out.length,
    events: limit > 0 ? out.slice(0, limit) : out,
  });
})();
