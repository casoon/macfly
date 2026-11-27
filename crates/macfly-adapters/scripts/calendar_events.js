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

const SKIP_SYSTEM_CALS = new Set([
  "deutsche feiertage",
  "geburtstage",
  "siri-vorschläge",
  "geplante erinnerungen",
  "birthdays",
  "siri suggestions",
  "scheduled reminders",
]);

(function () {
  const input = readInput();
  const days = Number(input.days || 7);
  const calendarFilter = input.calendar ? String(input.calendar).toLowerCase() : null;
  const limit = Number(input.limit ?? 50);

  const now = new Date();
  const startMs = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 0, 0, 0).getTime();
  const endMs = startMs + days * 24 * 60 * 60 * 1000;

  const Calendar = Application("Calendar");
  const cals = Calendar.calendars();
  const out = [];

  for (let i = 0; i < cals.length; i++) {
    const cal = cals[i];
    const calName = String(cal.name());
    const calLower = calName.toLowerCase();

    if (calendarFilter) {
      if (calLower !== calendarFilter) continue;
    } else if (SKIP_SYSTEM_CALS.has(calLower)) {
      continue;
    }

    // Batch property fetch (2 Apple Events per calendar instead of slow .whose evaluation)
    let starts = [];
    let summaries = [];
    try {
      starts = cal.events.startDate() || [];
      summaries = cal.events.summary() || [];
    } catch (_) {
      continue;
    }

    for (let j = 0; j < starts.length; j++) {
      const sd = starts[j];
      if (!sd) continue;
      const ms = new Date(sd).getTime();
      if (isNaN(ms) || ms < startMs || ms > endMs) continue;

      out.push({
        calendar: calName,
        title: String(summaries[j] || "(Ohne Titel)"),
        start_at: isoOrNull(sd),
        end_at: null,
        all_day: false,
        location: null,
      });
    }
  }

  out.sort((a, b) => (a.start_at || "").localeCompare(b.start_at || ""));

  return JSON.stringify({
    total_matched: out.length,
    events: limit > 0 ? out.slice(0, limit) : out,
  });
})();
