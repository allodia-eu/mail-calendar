# Undo and redo while writing: cross-platform contract

**Scope.** Taking back a change in text the user is writing, and putting it back: the composer's
message body, every field around it, and the signature editor, which hosts the same editor. Undoing
a mail action (the swipe undo bar) is a different thing and stays in
[`settings.md`](settings.md); a bulk action has none ([`list-selection.md`](list-selection.md)).

**Why it is a contract.** Undo is a reflex, not a feature anyone goes looking for. A user who
presses it and gets nothing, or gets half a change back, or gets the address they just removed
twice, stops trusting it everywhere. It has to behave the same on every platform, in every field,
for every kind of change.

## The rules

1. **Every place the user writes takes back its own changes.** Undo takes back the newest change,
   redo puts it back, and a new change leaves nothing to redo. Each field has its own history: the
   body has one, each recipient field one, Subject one. An undo in one never reaches another.

2. **The platform's own chords, in every field.** Ctrl+Z to undo, Shift+Ctrl+Z or Ctrl+Y to redo
   on Linux, Windows and Android with a keyboard; ⌘Z and ⇧⌘Z on macOS, and on an iPhone or iPad
   with a keyboard, where Control is left to the text system (Ctrl+Y is its yank). A layout with no
   Latin letter on the key still undoes: the chord is read off the key's letter on the first
   layout, as the toolkits' own bindings read it. Where the platform's own text fields offer Undo some other way (the macOS Edit
   menu, shaking an iPhone, the iPad's shortcut bar, the right-click menu of a Windows text box),
   the composer offers it the same way and it reaches the same history. Where they offer only the
   keyboard (GTK's text fields carry no Undo in their menu), so does the composer.

3. **The body's history is the editor's, never the engine's.** It is kept in the shared bundle,
   [`clients/composer/src/history.ts`](../clients/composer/src/history.ts), as the mutations each
   change made to the document, and an undo applies them backwards. A WebView's own undo stack
   holds only the edits its engine made itself, while most of what the editor does (lists, tables,
   links, pictures, the marks it rewrites after `execCommand`) is surgery on the document, which
   that stack never sees and which leaves its entries pointing at moved nodes. So it is not used:

   - The bundle answers the chords itself, in its own `keydown`. **No host binds them**, and a host
     must not bind them to anything else while the body has focus.
   - An Undo the engine runs from a menu or a gesture arrives as `beforeinput` with `historyUndo` or
     `historyRedo`, is cancelled, and is answered from the same history. A host whose platform
     offers such an entry point (rule 2) routes it there.
   - A host whose menu item reads the engine's stack to decide whether it can be chosen draws its
     own in its place instead, asking `composerHistorySteps()` and running `stepComposerHistory`.
     WebView2's right-click Redo is never enabled otherwise, because the engine never undid
     anything ([`composer-security.md`](composer-security.md), Gate 14).
   - A host whose platform asks that question synchronously, where a script cannot answer in
     time, is told instead. It announces the `history` notice, the bundle posts whether there is a
     step to take back and one to put back on Gate 2's channel each time that changes, and the
     host answers its platform from what it was last told and steps the history with
     `stepComposerHistory`. Apple's entry points all ask that way: the Edit menu, shaking, the
     three-finger gestures and the shortcut bar.

4. **A step in the body is one change, however it was made.** A toolbar command, a paste, a picture
   inserted or dropped, a list indented, a table inserted, a cell moved to, a link made or removed:
   one step each. Typing runs into one step until the caret moves, typing turns into deleting or the
   typing pauses for a second (`TYPING_PAUSE_MS`). Dragging a picture's corner is one step however
   many moves it took. The newest hundred steps are kept (`MAX_STEPS`).

5. **An undo puts the caret back too.** Undo restores the selection as it was before the change,
   redo the one after it. A formatting change undone shows the text it was applied to still
   selected.

6. **What the editor made of what was typed undoes to what was typed.** `- ` at the start of a line
   becomes a bullet and an address followed by a space becomes a link; undoing either leaves the
   characters the user typed, the space included, and only takes away what the editor did with
   them, as a word processor's autocorrect does.

7. **The history starts with the body the composer opened with.** A reopened draft, a quoted
   original, a mail link's body and an assistant's draft are where undo stops, not a step: they are
   seeded through `history.restart`, and the first Ctrl+Z after opening does nothing.

8. **A change the host makes from its own controls is not undone from the body.** The signature a
   From change swaps in, a quote style chosen in the composer, the quote attribution completed: each
   follows a native control, and undoing it from the body would leave the control saying one thing
   and the message another. They run through `history.outside`, and the user's steps on either side
   of them stay undoable. A step such a change has overtaken (its paragraph sat in front of a
   signature that is no longer in the message) is **refused whole and never half applied**, and the
   history starts again from there.

9. **A recipient field's history is the field's, a step per address.** Typing one address, finishing
   it with a comma or Enter, accepting a suggestion and removing a pill are one step each. The
   text entry inside the field keeps no history of its own: the field rewrites it each time an
   address is finished, so the entry's undo would type a finished address back in and the field
   would finish it a second time. Removing a pill puts the caret back in the field, so the chord
   that brings the address back is heard there. A field seeded by the composer (a reply's
   recipients) starts its history there.

10. **Subject and every other plain text field use the platform's own history.** The toolkit's
    text field already meets rules 1 and 2, except a Windows `TextBox`, which redoes on Ctrl+Y and
    not on Shift+Ctrl+Z: that one chord is added over the box's own history.

11. **A history lasts as long as its composer.** Sending, discarding and closing the composer end
    it. It is never stored, and a draft reopened later starts afresh (rule 7).

## Per-platform matrix

Every row is mandatory on every column. A cell that says *not yet* is open work with an issue,
never a resting state.

| Rule | macOS | iPhone, iPad | Windows | Android | Linux |
|---|---|---|---|---|---|
| Body: the chords (rules 2 to 6) | shared bundle; WebKit delivers ⌘Z and ⇧⌘Z to the page (verified by hand: typing, a pause, bold, a `- ` bullet, a link, a paste, a list, a table) | shared bundle, with a keyboard; WebKit delivers ⌘Z and ⇧⌘Z to the page (held by `ComposerUndoTests` on iPhone and iPad) | shared bundle; WebView2 delivers the chords to the page (verified by hand: typing, a pause, bold, a `- ` bullet, a link, a paste, a list, a table; Ctrl+Z, Ctrl+Y and Shift+Ctrl+Z held by `ComposerUndo.Tests`) | shared bundle; Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y verified on a Galaxy Note 20 with a Bluetooth keyboard | shared bundle; nothing in GTK binds the chords ahead of the web view, which delivers them to the page (verified by hand on a real keyboard: typing, a pause, bold, a `- ` bullet, a link, a paste, a list, a table) |
| Body: the platform's other entry points (rule 2) | the Edit menu's Undo and Redo, which AppKit validates against the window and not the first responder, so the web view takes `undo:` and `redo:` itself (`HistoryWebView`) and answers them from what the bundle told it (verified by hand: enabled exactly when there is a step, a table inserted from the toolbar included) | shaking, the three-finger gestures and the shortcut bar, which all ask the web view's `undoManager`: `EditorUndoManager`, answering from what the bundle told it (the shortcut bar and shaking verified by hand; the three-finger gestures ask the same manager and need a device) | the right-click menu's Undo and Redo, drawn by the host over the bundle's history (`EditorMenu`; held by `ComposerUndo.Tests`) | none: Android's text fields offer no Undo outside a keyboard | none: GTK's text fields offer no Undo in their menu |
| Recipient fields (rule 9) | `RecipientHistory`; the input is an `NSTextField` (`RecipientInput`) whose field editor answers `undo:` and `redo:` and the chords from it, and files its own typing nowhere; held by `RecipientHistoryTests`, and verified by hand with the chords and the Edit menu | `RecipientHistory`; the input is a `UITextField` (`RecipientInput`) whose `undoManager` is the field's, so its own typing is filed nowhere; held by `RecipientHistoryTests` and `ComposerUndoTests` | `RecipientHistory`, answered in the input's `PreviewKeyDown` ahead of the `TextBox`'s own, whose history is emptied as it fills; Undo and Redo in the input's right-click menu are the field's; held by `RecipientHistoryTests` and `ComposerUndo.Tests` | `RecipientHistory`, answered in the field's `onPreviewKeyEvent` ahead of the input's own; held by `RecipientHistoryTest` and `RecipientUndoTest`, and verified on a Galaxy Note 20 with a Bluetooth keyboard | `recipients::history`, the entry's own undo off (`set_enable_undo(false)`); held by the crate's GTK tests |
| Subject and other plain fields (rule 10) | `NSTextField` | `UITextField` | `TextBox`, plus Shift+Ctrl+Z (`HistoryKeys.RedoOnShiftChord`; held by `ComposerUndo.Tests`) | Compose's `TextField`, whose own history answers all three chords (verified on a Galaxy Note 20 with a Bluetooth keyboard) | `GtkEntry`, whose own bindings include Shift+Ctrl+Z and Ctrl+Y (verified on a real keyboard) |

## Enforcement

- `clients/composer/tests/history.test.ts` holds rules 3 to 8 for the body, against the real
  editing commands.
- Rule 9's step boundaries are held by `clients/linux/src/ui/recipients/history.rs`, Android's
  `RecipientHistoryTest`, Windows' `RecipientHistoryTests` and Apple's `RecipientHistoryTests`,
  and on the widget, chords included, by Linux's `recipients/field_tests.rs`, Android's
  `RecipientUndoTest`, Windows' `uitests/ComposerUndo.Tests.ps1`, which also holds the Windows
  right-click menus, and iPhone and iPad's `clients/apple/UITests/ComposerUndoTests.swift`.
- That a host leaves the chords to the page is held on Windows by `ComposerUndo.Tests`, which types
  into the body and reads it back through UI Automation, and on iPhone and iPad by
  `ComposerUndoTests`. Elsewhere it is verified by hand, on Linux in the headless session
  (`build-and-run.sh --headless`, then `control.sh linux key ctrl+z`).
  ⚠️ That session's synthetic keyboard does not trigger GTK's own Shift+Ctrl+Z in a plain
  `GtkEntry`, although the binding is in GTK's table and Ctrl+Y does; a Subject that will not redo
  there is the harness, not the field. On a real keyboard Subject redoes with both.
