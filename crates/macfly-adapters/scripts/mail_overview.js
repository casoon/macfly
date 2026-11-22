ObjC.import("Foundation");

function readInput() {
  const data = $.NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile;
  const raw = $.NSString.alloc.initWithDataEncoding(data, $.NSUTF8StringEncoding).js;
  return raw && raw.trim().length > 0 ? JSON.parse(raw) : {};
}

(function () {
  readInput();
  const Mail = Application("Mail");
  const accountsOut = [];
  const mailboxesOut = [];
  const signaturesOut = [];

  const accounts = Mail.accounts();
  for (let i = 0; i < accounts.length; i++) {
    const acc = accounts[i];
    const accId = String(acc.id());
    const accName = String(acc.name());
    let emails = [];
    try {
      emails = (acc.emailAddresses() || []).map((e) => String(e));
    } catch (_) {}

    // Gebatchter Abruf ohne teures messages.length über riesige Archive
    let mbNames = [];
    let mbUnreads = [];
    try {
      mbNames = acc.mailboxes.name() || [];
      mbUnreads = acc.mailboxes.unreadCount() || [];
    } catch (_) {}

    for (let j = 0; j < mbNames.length; j++) {
      mailboxesOut.push({
        name: String(mbNames[j]),
        account: accName,
        unread_count: Number(mbUnreads[j] || 0),
        message_count: 0,
      });
    }

    accountsOut.push({
      id: accId,
      name: accName,
      email_addresses: emails,
      mailbox_count: mbNames.length,
    });
  }

  try {
    const sigs = Mail.signatures();
    for (let s = 0; s < sigs.length; s++) {
      const sig = sigs[s];
      const name = String(sig.name());
      let content = "";
      try {
        content = String(sig.content() || "");
      } catch (_) {}
      const trimmed = content.trim();
      signaturesOut.push({
        name: name,
        content_preview: trimmed.length > 160 ? trimmed.slice(0, 160) + "…" : trimmed,
        char_count: trimmed.length,
      });
    }
  } catch (_) {}

  return JSON.stringify({
    accounts: accountsOut,
    mailboxes: mailboxesOut,
    signatures: signaturesOut,
  });
})();
