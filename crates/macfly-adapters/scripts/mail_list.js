ObjC.import("Foundation");

function readInput() {
  const data = $.NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile;
  const raw = $.NSString.alloc.initWithDataEncoding(data, $.NSUTF8StringEncoding).js;
  return raw && raw.trim().length > 0 ? JSON.parse(raw) : {};
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

function readStatusOf(msg) {
  try {
    return Boolean(msg.readStatus());
  } catch (_) {
    return true;
  }
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
  const accountFilter = input.account ? String(input.account).toLowerCase() : null;
  const mailboxFilter = input.mailbox ? String(input.mailbox).toLowerCase() : "inbox";
  const unreadOnly = Boolean(input.unread_only);
  const query = input.query ? String(input.query).toLowerCase() : null;
  const allowAccounts = (input.allow_accounts || []).map((s) => String(s).toLowerCase());
  const denyMailboxes = (input.deny_mailboxes || []).map((s) => String(s).toLowerCase());
  const limit = Number(input.limit || 25);
  const scanLimit = Number(input.scan_limit || 150);

  const Mail = Application("Mail");
  const out = [];
  const accounts = Mail.accounts();
  const inboxes = mailboxFilter === "inbox" ? inboxNames(Mail) : {};

  for (let i = 0; i < accounts.length; i++) {
    const acc = accounts[i];
    const accName = String(acc.name());
    const accLower = accName.toLowerCase();

    if (allowAccounts.length > 0 && !allowAccounts.includes(accLower)) continue;
    if (accountFilter && accLower !== accountFilter) continue;

    const mbs = acc.mailboxes();
    for (let j = 0; j < mbs.length; j++) {
      const mb = mbs[j];
      const mbName = String(mb.name());
      const mbLower = mbName.toLowerCase();

      if (denyMailboxes.includes(mbLower)) continue;
      if (
        mailboxFilter !== "*" &&
        !mailboxMatches(mailboxFilter, mbLower, accLower, inboxes)
      ) {
        continue;
      }

      // Fetch all message ids in one Apple Event (newest first, same order as
      // the index). Addressing messages by index makes Mail re-resolve the whole
      // mailbox per access (~1 s each on large mailboxes); by id it is ~ms.
      let ids = [];
      try {
        ids = mb.messages.id() || [];
      } catch (_) {
        ids = [];
      }
      if (ids.length === 0) continue;

      const toScan = Math.min(ids.length, scanLimit);

      // For a search, read subject and sender as whole columns (one Apple Event
      // each) instead of two events per scanned message; only matches are then
      // touched individually.
      let subjects = null;
      let senders = null;
      if (query) {
        try {
          subjects = mb.messages.subject();
          senders = mb.messages.sender();
          if (subjects.length !== ids.length || senders.length !== ids.length) {
            subjects = null;
            senders = null;
          }
        } catch (_) {
          subjects = null;
          senders = null;
        }
      }

      for (let k = 0; k < toScan; k++) {
        const msg = mb.messages.byId(ids[k]);

        // Without column data, check the cheap unread filter before fetching
        // subject and sender.
        let isRead = subjects ? null : readStatusOf(msg);
        if (unreadOnly && isRead) continue;

        let subject = "(Ohne Betreff)";
        let sender = "";
        if (subjects) {
          subject = String(subjects[k] || "(Ohne Betreff)");
          sender = String(senders[k] || "");
        } else {
          try {
            subject = String(msg.subject() || "(Ohne Betreff)");
          } catch (_) {}
          try {
            sender = String(msg.sender() || "");
          } catch (_) {}
        }

        if (
          query &&
          !subject.toLowerCase().includes(query) &&
          !sender.toLowerCase().includes(query)
        ) {
          continue;
        }

        if (isRead === null) {
          isRead = readStatusOf(msg);
          if (unreadOnly && isRead) continue;
        }

        const localId = String(ids[k]);
        let rfcId = null;
        let dateRecv = null;
        let flagged = false;
        try {
          const mid = msg.messageId();
          if (mid) rfcId = String(mid);
        } catch (_) {}
        try {
          dateRecv = isoOrNull(msg.dateReceived());
        } catch (_) {}
        try {
          flagged = Boolean(msg.flaggedStatus());
        } catch (_) {}

        out.push({
          local_id: localId,
          rfc_message_id: rfcId,
          account: accName,
          mailbox: mbName,
          subject: subject,
          sender: sender,
          date_received: dateRecv,
          read: isRead,
          flagged: flagged,
        });

        if (out.length >= limit * 2) break;
      }
    }
  }

  out.sort((a, b) => {
    const ad = a.date_received || "";
    const bd = b.date_received || "";
    return bd.localeCompare(ad);
  });

  return JSON.stringify({
    total_matched: out.length,
    messages: limit > 0 ? out.slice(0, limit) : out,
  });
})();
