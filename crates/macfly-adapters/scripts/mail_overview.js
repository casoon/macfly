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

  const accNames = Mail.accounts.name() || [];
  const accIds = Mail.accounts.id() || [];
  let accEmails = [];
  try {
    accEmails = Mail.accounts.emailAddresses() || [];
  } catch (_) {}
  const accObjs = Mail.accounts();

  for (let i = 0; i < accNames.length; i++) {
    const accName = String(accNames[i]);
    const accId = String(accIds[i] || accName);
    const emails = Array.isArray(accEmails[i])
      ? accEmails[i].map((e) => String(e))
      : [];

    let mbNames = [];
    let mbUnread = [];
    try {
      mbNames = accObjs[i].mailboxes.name() || [];
    } catch (_) {}
    try {
      mbUnread = accObjs[i].mailboxes.unreadCount() || [];
    } catch (_) {}

    for (let j = 0; j < mbNames.length; j++) {
      const name = String(mbNames[j]);
      mailboxesOut.push({
        name: name,
        account: accName,
        unread_count: Number(mbUnread[j] || 0),
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
    const sigNames = Mail.signatures.name() || [];
    for (let s = 0; s < sigNames.length; s++) {
      const name = String(sigNames[s]);
      signaturesOut.push({
        name: name,
        content_preview: "(Apple Mail native signature)",
        char_count: 0,
      });
    }
  } catch (_) {}

  return JSON.stringify({
    accounts: accountsOut,
    mailboxes: mailboxesOut,
    signatures: signaturesOut,
  });
})();
