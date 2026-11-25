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

// Apple Mail's own inbox name per account ("INBOX", "Posteingang", ...), keyed
// by lower-case account name, so a mailbox filter of "inbox" also finds
// localized inboxes such as Exchange's.
function inboxNames(Mail) {
  const map = {};
  try {
    const mbs = Mail.inbox.mailboxes();
    for (let i = 0; i < mbs.length; i++) {
      try {
        map[String(mbs[i].account.name()).toLowerCase()] = String(mbs[i].name()).toLowerCase();
      } catch (_) {}
    }
  } catch (_) {}
  return map;
}

function mailboxMatches(filter, mbLower, accLower, inboxes) {
  return mbLower === filter || (filter === "inbox" && inboxes[accLower] === mbLower);
}

(function () {
  const input = readInput();
  const targetLocalId = input.local_id ? String(input.local_id) : null;
  const accountHint = input.account ? String(input.account).toLowerCase() : null;
  const mailboxHint = input.mailbox ? String(input.mailbox).toLowerCase() : null;

  const id = Number(targetLocalId);
  if (!targetLocalId || !Number.isInteger(id)) {
    return JSON.stringify({ found: false });
  }

  const Mail = Application("Mail");
  const inboxes = mailboxHint === "inbox" ? inboxNames(Mail) : {};

  // Message ids are global in Apple Mail: byId on any mailbox resolves the
  // message wherever it lives (a few ms), while whose({id}) scans a mailbox
  // (seconds on large ones). Ask the message for its own mailbox first.
  let owner = null;
  try {
    const msg = Mail.accounts[0].mailboxes[0].messages.byId(id);
    if (msg.exists()) {
      const mb = msg.mailbox;
      owner = { msg: msg, accName: String(mb.account.name()), mbName: String(mb.name()) };
    }
  } catch (_) {
    owner = null;
  }
  if (!owner) return JSON.stringify({ found: false });

  const ownerAcc = owner.accName.toLowerCase();
  if (accountHint && ownerAcc !== accountHint) return JSON.stringify({ found: false });

  let hit = null;
  if (
    !mailboxHint ||
    mailboxMatches(mailboxHint, owner.mbName.toLowerCase(), ownerAcc, inboxes)
  ) {
    hit = owner;
  } else {
    // The same message can also sit in another mailbox of its account (e.g. a
    // Gmail label next to "All Mail"): check membership in the hinted mailbox
    // with one id column fetch per candidate.
    const acc = Mail.accounts.byName(owner.accName);
    let mbNames = [];
    try {
      mbNames = acc.mailboxes.name() || [];
    } catch (_) {}
    for (let j = 0; j < mbNames.length && !hit; j++) {
      const mbName = String(mbNames[j]);
      if (!mailboxMatches(mailboxHint, mbName.toLowerCase(), ownerAcc, inboxes)) continue;
      const mb = acc.mailboxes[j];
      let ids = [];
      try {
        ids = mb.messages.id() || [];
      } catch (_) {}
      if (ids.indexOf(id) !== -1) {
        hit = { msg: mb.messages.byId(id), accName: owner.accName, mbName: mbName };
      }
    }
  }
  if (!hit) return JSON.stringify({ found: false });

  const msg = hit.msg;
  let toList = [];
  let ccList = [];
  try {
    toList = msg.toRecipients.address().map((a) => String(a));
  } catch (_) {}
  try {
    ccList = msg.ccRecipients.address().map((a) => String(a));
  } catch (_) {}

  let body = "";
  try {
    body = String(msg.content() || "");
  } catch (_) {}

  let rfcId = null;
  try {
    const mid = msg.messageId();
    if (mid) rfcId = String(mid);
  } catch (_) {}

  return JSON.stringify({
    found: true,
    local_id: String(id),
    rfc_message_id: rfcId,
    account: hit.accName,
    mailbox: hit.mbName,
    subject: String(msg.subject() || "(Ohne Betreff)"),
    sender: String(msg.sender() || ""),
    to: toList,
    cc: ccList,
    date_received: isoOrNull(msg.dateReceived()),
    read: Boolean(msg.readStatus()),
    flagged: Boolean(msg.flaggedStatus()),
    body: body,
  });
})();
