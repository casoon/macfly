ObjC.import("Foundation");

function readInput() {
  const data = $.NSFileHandle.fileHandleWithStandardInput.readDataToEndOfFile;
  const raw = $.NSString.alloc.initWithDataEncoding(data, $.NSUTF8StringEncoding).js;
  return raw && raw.trim().length > 0 ? JSON.parse(raw) : {};
}

(function () {
  const input = readInput();
  const action = String(input.action || "");
  const Reminders = Application("Reminders");

  if (action === "reminders_create") {
    const listName = input.list ? String(input.list) : null;
    let targetList = Reminders.defaultList();
    if (listName) {
      const lists = Reminders.lists();
      const needle = listName.toLowerCase();
      for (let i = 0; i < lists.length; i++) {
        if (String(lists[i].name()).toLowerCase() === needle) {
          targetList = lists[i];
          break;
        }
      }
    }
    const props = { name: String(input.title || "Aufgabe") };
    if (input.notes) props.body = String(input.notes);
    const rem = Reminders.Reminder(props);
    targetList.reminders.push(rem);
    return JSON.stringify({
      ok: true,
      detail: "Erinnerung '" + props.name + "' erstellt in Liste '" + targetList.name() + "'",
    });
  }

  throw new Error("Unbekannte Reminders-Aktion: " + action);
})();
