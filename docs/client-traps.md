# Client traps

Platform behaviour that looks like a bug in our code and is not, or looks correct and is not. Each
one cost real time to find, so each is stated with what it breaks, how to prove it, and why the
obvious test does not see it.

These are not conventions. The rules a client must follow are in
[`AGENTS.md`](../AGENTS.md) → "Client conventions", and the per-surface contracts are the table in
that same file.

- **In a top-level `fun MainActivity.Foo()`, `this@MainActivity` does not resolve to the receiver.**
  Kotlin labels an extension function's implicit receiver with the *function's* name, so the label
  is `this@Foo`. Inside a real class member `this@MainActivity` works, which is exactly why moving
  one out to a sibling file breaks it. This matters because splitting `MainActivity` into
  `internal fun MainActivity.…` extensions is how its files are kept under the 500-line cap, so
  every such move meets this. The compiler catches it; the JVM suite does not, because the file
  never compiles far enough to run.

  A second cost of the same move: a `private` fixture that becomes `internal` is now visible to
  every other test in the package, and generic names (`NOW`, `block`, `page`) collide with
  identically-named privates elsewhere. Prefix them for the file they came from.
- **A `*/` inside a Rust doc comment on a UniFFI-exported item breaks the generated Kotlin.**
  UniFFI copies our doc text into the bindings, and Kotlin's are `/** … */` blocks, so the first
  `*/` in the prose closes the comment and the rest of it becomes top-level Kotlin. Writing that a
  wildcard media type is refused, with the wildcard spelled out, cost an hour: `mailcal-bindings`
  compiled, `cargo doc` was clean, the C# generator was happy, and only `:app:test` failed, 200
  lines into a 20,000-line generated file, with `Parameter name expected`. Swift and C# take `///`
  line comments and never see it, which is what makes this Kotlin's alone. Describe the sequence
  rather than typing it.
- **`Path.GetInvalidFileNameChars()` answers differently per host, and `Mailcal.Tests` is not a
  Windows assembly.** On Windows it returns the familiar set; on Linux, `/` and NUL alone, so `:`,
  `*`, `?` and `\` all come back as legal. The Windows client only ever *runs* on Windows, but its
  unit suite is plain `net10.0` and the gate runs it on whatever host is to hand, so a rule built
  on that call passes on one and fails on the other, and the test cannot state what the rule is.
  Name the reserved set explicitly. The same caution applies to any `Path`, `Environment` or
  culture API whose answer is the host's: in that assembly, the host is not the product's.
- **In the Android composer's WebView, viewport units compute to `0px`.** Compose lays an
  `AndroidView` out *after* the page loads, so Chromium fixes the layout viewport at zero height and
  `100vh` / `100%` silently do nothing. Size from `document.documentElement.clientHeight` and
  re-measure on `resize` (`fillViewport` in [`editor.html`](../clients/composer/dist/editor.html)).
  A WebView layout bug cannot be caught by the JVM suite: Robolectric has no renderer; prove it
  against a real WebView (`adb forward` to `webview_devtools_remote_<pid>` + CDP
  `Runtime.evaluate`).
- **Each column of a `NavigationSplitView` reports its own `horizontalSizeClass`.** Read from
  inside a list row, an iPad's *list column* answers `.compact` while the window is regular, so
  any "is this the phone layout?" test written at the row decides the opposite of the one written
  at `ContentView`. Read it at the view that owns the window and pass the answer down;
  `ContentView.hasReadingPane` is that answer. The failure looks like a platform bug rather than a
  logic error: the same code draws correctly on macOS and on iPhone, and wrongly only on iPad,
  where the size class is the only one of the three that is per-column.

- **On iOS/iPadOS a `sheet` or `fullScreenCover` does not follow the app's light/dark setting.**
  `preferredColorScheme` travels *up* to a hosting controller, and a modal presentation gets its
  own: it copies the presenter's scheme once, at presentation, and never again, so the screen the
  Appearance picker lives on keeps the old scheme while everything behind it repaints. Restate the
  scheme on the presented content, and restate it as an **explicit** value, never `nil`: releasing
  the preference leaves that controller on whatever it last resolved to, so "Use system setting"
  repaints once and then ignores the host for as long as the modal stays open. ContentView's
  `windowScheme` is the value to hand over: it already carries both cases. macOS needs none of
  this; a sheet shares the presenter's window and follows it live. Neither half is visible to
  `swift test` (there is no Apple UI-test target): prove it on a simulator whose own appearance is
  the *opposite* of the one under test, driving the picker with `idb` and flipping the host with
  `xcrun simctl ui <udid> appearance light|dark`.
- **On Linux, hand a URI or a file to the desktop through the portal launchers, never through
  `AppInfo`.** `gtk::UriLauncher` for a URI, `gtk::FileLauncher` for a file
  (`check-desktop-handoff.sh` catches the shapes a grep can decide).

  `g_app_info_launch_default_for_uri` resolves against the **desktop's application database**. A
  Flatpak has none, so GIO falls back through GVFS onto the session bus, and the call is
  **synchronous, on the GTK main thread**. Measured on the shape that ships: clicking a sign-in
  button froze the entire app, with no repaint and not even the flow's own Cancel button left to
  press. A file is worse than a URI, because the path is one inside the sandbox's own filesystem
  view; the portal takes a **file descriptor**, which is what `FileLauncher` sends.

  **The trap is that it works while you develop it.** `build-and-run.sh --host` builds against the
  distribution's GTK, outside the sandbox, where the application database is right there, so the
  bug is invisible in the loop everyone iterates in, and appears only in the packaged build. No test
  can see it either: reproducing it needs a sandbox and a live portal.

  Both launchers are asynchronous, so the failure arrives in a callback rather than as a return
  value. Route it through an `AppInput`, the way the attachment path does: a launch that fails
  after the flow has started waiting still has to put that flow back.

  **And the file has to be somewhere the portal can pass on**, which `std::env::temp_dir()` is not:
  `/tmp` inside a Flatpak is the sandbox's own tmpfs, invisible to the host and to whatever
  application the portal launches. Handing over a descriptor from it fails with nothing more useful
  than *"The application launch failed"*, which reads as the file being broken. Write it under
  `glib::user_cache_dir()`, a real host path in both shapes. Same trap as the API: correct on
  `--host`, wrong only where it ships.
- **On Linux, `ashpd`'s session-bus connection is process-global, so no portal caller may own its
  own Tokio runtime.** `ashpd` caches one `zbus::Connection` in a `OnceLock`, and zbus drives that
  connection from whichever runtime opened it. A runtime built per call, or owned by one service,
  takes the connection's reader with it when it is dropped while the connection itself stays
  cached: **every later portal call then awaits a reply that can never arrive**. No error, no
  timeout, a thread parked for the life of the process, and whatever state machine that thread was
  serving parked with it.

  Two services here reach the portal, and neither one looks like the other's problem: new-mail
  notifications, and the secure store, because inside a sandbox `oo7` asks
  `org.freedesktop.portal.Secret` for the keyring key. A store whose keyring **fails** to open is
  the nastiest shape, since it seeds the shared connection and then drops its runtime on the way
  out, so the first notification of the session hangs and the cause is three files away. Everything
  goes through [`host_runtime`](../clients/linux/src/host_runtime.rs), which owns the one runtime
  and never drops it; `check-portal-runtime.sh` refuses a second one anywhere else in the client.

  **The tell is that it works exactly once.** The first portal call of the process succeeds, so a
  manual check passes and a screenshot proves nothing; only the second one hangs. Bound a portal
  exchange nobody is waiting on, so one that stops answering costs a single pass rather than the
  session: `notifications::post` does. Unlocking the keyring is the exception and stays unbounded,
  because it legitimately waits on the desktop's own password prompt.
- **Linux libadwaita rows parse titles _and subtitles_ as Pango markup by default.**
  `adw::ActionRow` / `PreferencesRow` text may hold localised ampersands or untrusted subjects, so
  set `use_markup(false)` unless the string was deliberately produced as escaped markup. A row that
  never sets it renders **blank** on a bare ampersand and *applies* a markup-shaped subject
  (`<b>Wire transfer</b>` arrives styled): a security gate, not a cosmetic one.

  **Set it with a setter, before the text: the property builder cannot promise the order.**
  `g_object_new` applies properties in *its* order, so `.title(…).subtitle(…).use_markup(false)`
  sets the subtitle while markup is still on. libadwaita re-applies the labels when the flag flips,
  so the row still *reads* correctly and every rendering assertion passes, but the markup-parsed
  first attempt has already logged `Failed to set text … from markup` for every sender or subject
  with an ampersand, into the diagnostic log a user attaches to a support request. Build via
  `plain_text_row()` ([`mailbox.rs`](../clients/linux/src/ui/mailbox.rs)) and `set_title` /
  `set_subtitle` after.

  **Assert on the rendered label, not the property.** `ActionRow::title()` returns the string you
  handed it whatever the label did: a green assertion for a blank row. Walk to the `GtkLabel`; for
  the ordering half, capture `glib::log_set_default_handler` and assert no `from markup` record,
  because rendering alone cannot see it. Put the ampersand in **both** the subject and the sender:
  the builder's order put `use-markup` after `title` but before `subtitle`, so a subject-only
  fixture stays silent while real mail warns. Keep the GTK widget regression test, and exercise
  both `--account demo` and the Stalwart fixture when changing Linux row rendering.

  **An explicit accessible label *or description* on one of these rows is silently ignored.**
  `AdwActionRow` publishes its title through a `labelled-by` relation **and its subtitle through a
  `described-by` one**, and by the ARIA rules GTK follows a relation beats the matching explicit
  property, so on a row that has a subtitle, both `AccessibleProperty::Label(…)` and
  `AccessibleProperty::Description(…)` change nothing a screen reader hears. Supplementary text
  therefore reaches the user only as the **subtitle**; `Description` is a live lever only on a row
  with no subtitle, and on a plain widget (a `GtkButton`), which has no competing relation.

  **A row's description is a relation, so it is not in the `description` field.** Measured on the
  shipped runtime: the card row reports `description=''` while carrying
  `ATSPI_RELATION_DESCRIBED_BY → [label "Keeps your list of mail accounts…"]`. So the AT-SPI dump's
  `description=` column is **empty for every row**, and reading it is how you conclude a working
  row is broken. Assert `getRelationSet()` for a row; `--description` is right only for the plain
  widgets it was written for (the invitation buttons). **GTK exposes no getter for any of it**, so
  a widget test cannot see this at all: the only oracle is an AT-SPI run
  (`scripts/dev/test-linux-ui.sh`), which is where the assertion belongs.
- **A key controller on a `GtkEntry` must be in the `Capture` phase to see Return.** The entry
  claims it first for its own `activate`, so a bubble-phase handler, the default, never gets it,
  while Down, Up and Escape arrive normally. The result is a keyboard path that is *half* working:
  arrows move the selection in a completion list and Enter moves focus instead of accepting, which
  reads as "the list ignores me". Every other key still passes through, so `Capture` costs nothing.
  Delivering a real key needs an event loop, so assert the **phase**
  (`observe_controllers()` → `propagation_phase()`): the phase is the defect.
- **A row belongs in a list box, and one that does not is skipped by the keyboard.**
  `AdwActionRow`, `AdwSwitchRow`, `AdwEntryRow`, `AdwComboRow` and `AdwExpanderRow` are all
  `GtkListBoxRow` subclasses. Appended to a plain `GtkBox` a row still *renders*, so every
  rendering assertion passes, but GTK's focus walk reaches it, `gtk_list_box_row_grab_focus`
  fails its own precondition, and the row and the control it carries are never focused. An
  `AdwPreferencesGroup` supplies the list. `every_row_belongs_to_a_list`
  ([`mailbox_tests.rs`](../clients/linux/src/ui/mailbox_tests.rs)) asserts a whole window at once:
  call it from any widget test that presents one.
- **A GLib critical is diagnosed with a backtrace, never by reading widget code.** The message is
  raised by a check deep inside the toolkit that knows nothing about what you did, so reasoning
  from its wording to a cause produces a plausible theory and the wrong file.
  `scripts/dev/gtk-trace.sh` ([`diagnose-linux-gtk`](.agents/skills/diagnose-linux-gtk)) gets the
  stack: `test` for the widget suite, `probe` for the running app, `dbus` for anything that
  stalls for twenty-five seconds, which is GIO's call timeout rather than a hang.
  ⚠️ **The host's toolkit is not the one that ships**, and they disagree about focus: a critical
  the app raises need not reproduce under `cargo test`, and the suite can raise the same message
  from an entirely different place. Confirm the backtrace names the site you care about before
  changing anything, and finish on the shipped toolkit.

## Interaction quality is not testable from a chair

- **A synthetic swipe cannot reproduce the bugs that matter.** `adb input swipe`, and any test that
  waits for the UI to settle, tests the case that already worked. Failures live where a *hand* goes:
  a finger arriving while the previous gesture's animation is still running. Reproduce it in a JVM
  test by taking the clock from Compose (`mainClock.autoAdvance = false`) and delivering the next
  gesture one frame later.
- **The two obvious performance instruments answer confidently wrong.** `gfxinfo`'s "Janky frames %"
  is a ratio over two different denominators, and `mpdecimate` over a `screenrecord` rates a fixed
  build worse than a broken one (the recorder caps at 60fps and perturbs the app). Measure the **gaps
  between frames during motion** from `dumpsys gfxinfo <pkg> framestats`, on a **release** build.
  Record video to see behaviour; measure timing with framestats.
- **The developer's phone holds their real mail and diary.** A horizontal `input swipe` on the mail
  list is a swipe *action* and archives real messages: assert the right screen is showing before
  every injection, and `svc power stayon usb`. A signature mismatch on install is a *safe* failure;
  "fixing" it with `adb uninstall` deletes their accounts and store: ask first. When tracing against
  real data, log counts and durations, **never** content.
