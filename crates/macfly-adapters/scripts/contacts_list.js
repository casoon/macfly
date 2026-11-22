ObjC.import("Foundation");

function readInput() {
  const data = $.NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile;
  const raw = $.NSString.alloc.initWithDataEncoding(data, $.NSUTF8StringEncoding).js;
  return raw && raw.trim().length > 0 ? JSON.parse(raw) : {};
}

(function () {
  const input = readInput();
  const query = input.query ? String(input.query).toLowerCase() : null;
  const limit = Number(input.limit || 0);

  const Contacts = Application("Contacts");
  const people = Contacts.people;
  const count = people.length;
  if (count === 0) {
    return JSON.stringify({ total_matched: 0, contacts: [] });
  }

  // Batch property fetch across all contacts
  const ids = people.id();
  const names = people.name();
  let orgs = [];
  try {
    orgs = people.organization();
  } catch (_) {
    orgs = new Array(count).fill(null);
  }
  let emailsList = [];
  try {
    emailsList = people.emails.value();
  } catch (_) {
    emailsList = new Array(count).fill([]);
  }
  let phonesList = [];
  try {
    phonesList = people.phones.value();
  } catch (_) {
    phonesList = new Array(count).fill([]);
  }

  const out = [];
  for (let i = 0; i < count; i++) {
    const name = String(names[i] || "").trim();
    const org = orgs[i] ? String(orgs[i]).trim() : null;
    const emails = Array.isArray(emailsList[i])
      ? emailsList[i].filter(Boolean).map((e) => String(e).trim())
      : [];
    const phones = Array.isArray(phonesList[i])
      ? phonesList[i].filter(Boolean).map((p) => String(p).trim())
      : [];

    const displayName = name || org || (emails[0] || "(Unbenannt)");

    if (query) {
      const hay = (
        displayName +
        " " +
        (org || "") +
        " " +
        emails.join(" ") +
        " " +
        phones.join(" ")
      ).toLowerCase();
      if (!hay.includes(query)) continue;
    }

    out.push({
      id: String(ids[i]),
      name: displayName,
      organization: org && org.length > 0 ? org : null,
      emails: emails,
      phones: phones,
    });
  }

  out.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));

  return JSON.stringify({
    total_matched: out.length,
    contacts: limit > 0 ? out.slice(0, limit) : out,
  });
})();
