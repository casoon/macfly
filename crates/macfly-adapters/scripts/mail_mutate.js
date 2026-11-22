ObjC.import("Foundation");

function readInput() {
  const data = $.NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile;
  const raw = $.NSString.alloc.initWithDataEncoding(data, $.NSUTF8StringEncoding).js;
  return raw && raw.trim().length > 0 ? JSON.parse(raw) : {};
}

function findMailbox(Mail, accountName, mailboxName) {
  const accounts = Mail.accounts();
  const accNeedle = String(accountName).toLowerCase();
  const mbNeedle = String(mailboxName).toLowerCase();
  for (let i = 0; i < accounts.length; i++) {
    if (String(accounts[i].name()).toLowerCase() !== accNeedle) continue;
    const mbs = accounts[i].mailboxes();
    for (let j = 0; j < mbs.length; j++) {
      if (String(mbs[j].name()).toLowerCase() === mbNeedle) {
        return { account: accounts[i], mailbox: mbs[j] };
      }
    }
  }
  throw new Error("Postfach nicht gefunden: " + accountName + "/" + mailboxName);
}

(function () {
  const input = readInput();
  const Mail = Application("Mail");
  const action = String(input.action || "");

  if (action === "mail_create_draft") {
    let bodyText = String(input.body || "");

    // Single Point of Truth: Signatur direkt aus Apple Mail oder Inline-Override
    let sigObj = null;
    if (input.signature_source === "apple-mail") {
      try {
        const sigs = Mail.signatures();
        if (sigs.length > 0) {
          if (input.signature_name) {
            const needle = String(input.signature_name).toLowerCase();
            for (let s = 0; s < sigs.length; s++) {
              if (String(sigs[s].name()).toLowerCase() === needle) {
                sigObj = sigs[s];
                break;
              }
            }
          } else {
            sigObj = sigs[0];
          }
        }
      } catch (_) {}
    } else if (input.signature_source === "inline" && input.signature_text) {
      bodyText = bodyText + "\n\n" + String(input.signature_text);
    }

    const msg = Mail.OutgoingMessage({
      subject: String(input.subject || ""),
      content: bodyText,
      visible: true,
    });
    Mail.outgoingMessages.push(msg);

    const toList = input.to || [];
    for (let i = 0; i < toList.length; i++) {
      msg.toRecipients.push(Mail.Recipient({ address: String(toList[i]) }));
    }
    const ccList = input.cc || [];
    for (let i = 0; i < ccList.length; i++) {
      msg.ccRecipients.push(Mail.Recipient({ address: String(ccList[i]) }));
    }

    if (sigObj) {
      try {
        msg.messageSignature = sigObj;
      } catch (_) {}
    }

    // Bewusst NUR als Entwurf geöffnet — KEIN msg.send()!
    return JSON.stringify({
      ok: true,
      detail:
        "Mail-Entwurf erstellt (Betreff: '" +
        input.subject +
        "', Empfänger: " +
        toList.join(", ") +
        ", Signatur: " +
        (sigObj ? sigObj.name() : input.signature_source) +
        ") — bereit zur Prüfung in Apple Mail",
    });
  }

  if (action === "mail_mark_read") {
    const found = findMailbox(Mail, input.account, input.mailbox);
    const matches = found.mailbox.messages.whose({ id: Number(input.local_id) })();
    if (!matches || matches.length === 0) {
      throw new Error("Mail nicht gefunden: " + input.local_id);
    }
    matches[0].readStatus = Boolean(input.read);
    return JSON.stringify({
      ok: true,
      detail:
        "Mail " + input.local_id + " als " + (input.read ? "gelesen" : "ungelesen") + " markiert",
    });
  }

  if (action === "mail_move") {
    const src = findMailbox(Mail, input.account, input.from_mailbox);
    const dst = findMailbox(Mail, input.account, input.to_mailbox);
    const matches = src.mailbox.messages.whose({ id: Number(input.local_id) })();
    if (!matches || matches.length === 0) {
      throw new Error("Mail nicht gefunden: " + input.local_id);
    }
    Mail.move(matches[0], { to: dst.mailbox });
    return JSON.stringify({
      ok: true,
      detail:
        "Mail " + input.local_id + " verschoben: " + input.from_mailbox + " → " + input.to_mailbox,
    });
  }

  throw new Error("Unbekannte Mail-Aktion: " + action);
})();
