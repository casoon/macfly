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
      if (mailboxFilter !== "*" && mbLower !== mailboxFilter) continue;

      const total = mb.messages.length;
      if (total === 0) continue;

      const toScan = Math.min(total, scanLimit);
      for (let k = 0; k < toScan; k++) {
        const msg = mb.messages[k];
        let isRead = true;
        try {
          isRead = Boolean(msg.readStatus());
        } catch (_) {}
        if (unreadOnly && isRead) continue;

        let subject = "(Ohne Betreff)";
        let sender = "";
        try {
          subject = String(msg.subject() || "(Ohne Betreff)");
        } catch (_) {}
        try {
          sender = String(msg.sender() || "");
        } catch (_) {}

        if (
          query &&
          !subject.toLowerCase().includes(query) &&
          !sender.toLowerCase().includes(query)
        ) {
          continue;
        }

        let localId = "";
        let rfcId = null;
        let dateRecv = null;
        let flagged = false;
        try {
          localId = String(msg.id());
        } catch (_) {}
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
