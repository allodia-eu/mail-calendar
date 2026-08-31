---
name: diagnose-linux-gtk
description: Find the cause of a GLib/GTK critical, warning or hang in the Linux client, the ones that name a failed assertion and nothing about the code that reached it. Wraps scripts/dev/gtk-trace.sh (gdb backtrace, D-Bus call trace, a backtrace probe in the running app, leaked bus names). Use when the diagnostic log or stderr carries a Gtk-CRITICAL / GLib-GObject-CRITICAL, when a widget behaves as if it is not there, or when something on this client hangs for about 25 seconds. Covers the false answers that cost hours.
---

# diagnose-linux-gtk: turn a GLib diagnostic into a line of our code

To *boot* a client see **debug-app**; to drive the shipped toolkit semantically see
`scripts/dev/test-linux-ui.sh`. This skill is about the messages those produce and where they
came from.

A GLib critical is a failed precondition **inside** the toolkit. It names the assertion and the
function that checked it, and nothing at all about the code that got there. Worse, the widget
usually still renders, so every rendering assertion passes, the screen looks right, and the only
thing wrong is behaviour nobody is testing. `crash.rs` puts these in the user's diagnostic log
([`docs/logging.md`](../../../docs/logging.md)), which is where you will usually meet one.

## The rule

**Get a backtrace. Do not read widget code.**

The temptation is to reason from the assertion's name to a plausible cause. The message is
generated deep in the toolkit by a check that has no idea what you did, so a plausible cause is
usually a wrong one, and it is expensive: you go and read the code that *looks* related, build a
theory that fits, and only find out it is wrong after changing something.

```sh
scripts/dev/gtk-trace.sh test                                 # whole widget suite
scripts/dev/gtk-trace.sh test ui::mailbox::tests::gtk_rows    # one test
```

`G_DEBUG=fatal-criticals` under gdb: the first critical aborts, so you are standing in the frame
that raised it, and the dev profile's line tables name our frames with files and lines. The frame
you want is the first one in `clients/linux/`.

## Four false answers

Each of these has already cost real time.

**1. The host's toolkit is not the one that ships.** `cargo test -p mailcal-linux` links the
distribution's GTK; the app links the **runtime's** (`scripts/dev/sdk.sh versions` prints both).
They disagree about focus in particular, so a critical the running app raises need not reproduce
in the host suite at all. If `test` comes up empty, that is not an answer: go to `probe`.

**2. The same message from two different places.** The host suite and the running app can each
raise a given critical from a *different* site: one in the product, one in a test helper.
Confirm the backtrace names the path you actually care about before fixing anything, or you will
fix the helper and ship the bug.

**3. A handler you installed is silently gone.** GLib offers no way to read the handler currently
installed, so `glib::log_unset_default_handler()` restores **GLib's** default rather than whatever
was there before. `glib_records()`
([`mailbox_tests.rs`](../../../clients/linux/src/ui/mailbox_tests.rs)) ends with exactly that call,
so a probe you install beside it stops firing the first time a test captures records, and the run
goes green on a bug that is still happening. This is why the modes above use gdb and `crash.rs`
rather than a handler of their own; do not hand-roll one.

**4. A twenty-five second failure is not a hang.** That is GIO's default D-Bus call timeout, so
something is waiting on a peer that never answers.

```sh
scripts/dev/gtk-trace.sh dbus <filter>     # the last call before the stall is the one
```

The common cause here is a **leaked run of a widget test** still owning the application id it
registered: the next run becomes GApplication's *remote* instance and blocks talking to a process
that serves nothing. One such process squatted for 17 hours and made every Linux suite run fail at
registration with `Timeout was reached`, which reads exactly like a broken session bus and is not.

```sh
scripts/dev/gtk-trace.sh squatters         # then kill what it names
```

Tests register `NON_UNIQUE` so this cannot bite them; anything that does not is exposed.

## When it only happens in the running app

Attach a backtrace to the record `crash.rs` already writes, then reproduce however you like. The
probe is removed however the command exits.

```sh
scripts/dev/gtk-trace.sh probe 'box != NULL' -- scripts/dev/sdk.sh cargo build -p mailcal-linux
scripts/dev/test-linux-ui.sh --no-build
grep -A 30 GTKTRACE target/ui-test-artifacts/linux/*/xdg-data/mailcal/mailcal.log
```

Building inside `probe` and running the suite after it is deliberate: the probe only has to be
present in the **binary**, and the acceptance suite is the cheapest way to drive the app through
enough of itself to raise the critical.

## Fixing it, and proving the fix

The log is not the defect. Find what the user loses, because that decides whether this is a
changelog entry and it is usually not obvious from the message:

- Drive the actual behaviour. Keyboard reachability is `window.child_focus(DirectionType::TabForward)`
  in a loop, reading back `GtkWindowExt::focus(&window)`: that is what showed a consent toggle
  was mouse-only.
- **Assert on structure or behaviour, never on the property you set.** `ActionRow::title()` hands
  back the string you gave it whatever the label did, so a property assertion is a green light for
  a blank row.
- A regression test that cannot fail is worse than none. Revert the fix, watch the test fail, put
  it back. Always.
- Finish on the shipped toolkit: `scripts/dev/test-linux-ui.sh` and diff the app's own log for the
  message, which is the only proof that reaches what the user runs.

## Rows, specifically

`AdwActionRow`, `AdwSwitchRow`, `AdwEntryRow`, `AdwComboRow` and `AdwExpanderRow` are all
`GtkListBoxRow` subclasses, and one appended to a plain `GtkBox` belongs to no list. It renders;
GTK's focus walk then reaches it, `gtk_list_box_row_grab_focus` fails its precondition, and the
row is skipped. An `AdwPreferencesGroup` supplies the list, and `every_row_belongs_to_a_list`
([`mailbox_tests.rs`](../../../clients/linux/src/ui/mailbox_tests.rs)) asserts a whole window's
worth at once. Call it from any widget test that presents one, as the welcome, About and
appearance cases already do.
