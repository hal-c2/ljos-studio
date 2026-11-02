# Field notes

A small notes app for the phone frame, made to dogfood Studio
(docs/DOGFOOD.md). It shows:

- four screens in a `StackView`, with push and pop transitions timed by
  `Motion`: the list (`ListScreen.qml`), a note (`DetailScreen.qml`), the
  add and edit form (`EditScreen.qml`) and settings (`SettingsScreen.qml`);
- the list read from `entries.json` by a `JsonListModel` (`EntryModel.qml`,
  as `document_new` with `data` writes it). The screens edit
  that model; `entries.json` is read again when the app starts;
- form validation: a title and a place are required and the date must read
  `YYYY-MM-DD`; Save is enabled only when the form is valid, and each field
  says what is wrong with it;
- states and transitions: the list's `empty` state fades in a hint when
  there are no notes, and the form's `editing` state retitles it;
- light and dark themes from `Ljos.Design` tokens. `theme.json` sets the
  seed hue, and the switch in Settings sets `Colors.dark`.

Open it in Studio with `studio samples/notes` (or `mise run studio
samples/notes`), choose Phone, and open `Main.qml`; Play lets the canvas take
clicks. Run it on its own with `ljos run samples/notes/Main.qml`.
