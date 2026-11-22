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
  const targetLocalId = input.local_id ? String(input.local_id) : null;
  const accountHint = input.account ? String(input.account).toLowerCase() : null;
  const mailboxHint = input.mailbox ? String(input.mailbox).toLowerCase() : null;

  if (!targetLocalId) {
    return JSON.stringify({ found: false });
  }

  const Mail = Application("Mail");
  const accounts = Mail.accounts();

  for (let i = 0; i < accounts.length; i++) {
    const acc = accounts[i];
    const accName = String(acc.name());
    if (accountHint && accName.toLowerCase() !== accountHint) continue;

    const mbs = acc.mailboxes();
    for (let j = 0; j < mbs.length; j++) {
      const mb = mbs[j];
      const mbName = String(mb.name());
      if (mailboxHint && mbName.toLowerCase() !== mailboxHint) continue;

      let matches = [];
      try {
        matches = mb.messages.whose({ id: Number(targetLocalId) })();
      } catch (_) {
        matches = [];
      }

      if (!matches || matches.length === 0) continue;
      const msg = matches[0];

      let toList = [];
      let ccList = [];
      try {
        toList = msg.toRecipients().map((r) => String(r.address()));
      } catch (_) {}
      try {
        ccList = msg.ccRecipients().map((r) => String(r.address()));
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
        local_id: String(msg.id()),
        rfc_message_id: rfcId,
        account: accName,
        mailbox: mbName,
        subject: String(msg.subject() || "(Ohne Betreff)"),
        sender: String(msg.sender() || ""),
        to: toList,
        cc: ccList,
        date_received: isoOrNull(msg.dateReceived()),
        read: Boolean(msg.readStatus()),
        flagged: Boolean(msg.flaggedStatus()),
        body: body,
      });
    }
  }

  return JSON.stringify({ found: false });
})();
