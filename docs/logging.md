# Diagnostic logging: cross-platform contract

**Scope.** How every Allodia Mail & Calendar client keeps a local diagnostic log. A field issue
(an empty calendar, a stuck sync, a boot hang) is diagnosed from one file the user can hand us,
so logging is a **first-class, consistent** feature, not a per-client afterthought. This is the
**single bar** all clients meet: Apple (macOS, iOS, iPadOS), Windows, Android, Linux, and any future
platform. Every shipping client also **surfaces** the log in-app (Settings → Diagnostics,
below) so a user can view and hand it over without a cable; auto-attaching it to a support
ticket remains future work (there is no support backend yet).

**Principle.** The log is **capped, rotating, and privacy-safe** on every platform. Adding the
surface on a new platform means meeting every cell of the matrix below; see the enforcement rule
at the end and in [`../AGENTS.md`](../AGENTS.md).

## The port (shared) · `crates/mailcal-bindings/src/logging.rs`

The product core and its binding layer emit records through the lightweight [`log`] facade
(`log::info!`, `log::debug!`, …) with **no knowledge of where they go**. A host passes a `Logger`
callback in at construction (`MailcalApp::new_accounts`); a process-wide `LogBridge` forwards
every record to it as `(level, target, message)`. **One log, every layer**: the shared Rust
core, the bindings, and the native host land in one stream.

- **Level gating is shared.** The host sets the ceiling at boot; a suppressed record is dropped in
  the `log` macros and **never crosses the FFI**, so raising verbosity for a support session
  (`MailcalApp::set_log_level`) costs nothing when off.
- **`debug` is ours; below it, everyone's.** Raising the level to `debug` is a support action
  (*show me what the app did*), and a dependency answers that question with its own internals, so
  `debug` and `trace` are filtered to this product's crates and the engine beneath them. The HTML
  parser emits a record **per text node**: one sync over a real mailbox fills the rotating cap
  with parser state and evicts the lines the session was enabled to capture, having cost real time
  formatting each one and crossing it to the host. Nothing is filtered at `info` and below: a
  dependency's `warn` is exactly what a support log should keep.
- **Connection facts are shared.** After a live provider connects at boot, after a disconnected
  boot placeholder reconnects, and after a new account is added, the bindings log that provider's
  `ConnectionInfo`: account type, provider count, capability flags, and observed TLS/HTTP versions.
  The label is only an account position (`account[0]`) or lifecycle label (`new-account`) plus
  provider bucket/index; never an account id, address, username, host, endpoint, or credential.
- **The connect *phase* is traced, and its URLs are not.** `ConnectionInfo` above reports a connect's
  *outcome*; the engine's `ConnectObserver` seam reports the steps that got there, and
  `mailcal-account`'s `connect_log.rs` carries one on every IMAP / JMAP / CalDAV config, so the
  sync provider, the `IDLE` watcher, and each re-dial are all traced, and a failed connect says
  *where* it broke (a redirect walked off, TLS never came up, credentials refused) instead of one
  opaque error. Each step is logged as the **event** it is (`connect[imap]: authenticated`) and its
  **URL is dropped**: a step's payload is a host/endpoint, which the rule below forbids, and the
  engine's scrub only strips `userinfo`: a CalDAV calendar-home href is routinely
  `/calendars/<address>/`, so an `@` in the *path* would otherwise write the user's own address
  into the attachable log. Do not "enrich" these lines with the URL. (Graph emits no steps: its
  connect performs no I/O.) The one payload logged **in full** is the negotiated dialect:
  `connect[imap]: IMAP4rev2, extensions: IDLE, LIST-STATUS, SPECIAL-USE`, because a protocol
  dialect and its capability atoms name the *server's software*, never the account, and because it
  is the line that explains why two accounts on one build behave differently. It reports what the
  session may **use**, so a rev2 account lists the extensions that dialect folded in even though its
  server advertised none of them separately.
- **The account lifecycle is narrated, including the credential write.** Adding an account, a
  refused connect, a token rotation, a removal and an erase each write a line, so a support log
  answers "did this account's credential ever reach this device?" rather than leaving it to be
  inferred. That inference used to be the only evidence there was: a *successful* store wrote
  nothing at all, which is the same unfalsifiable shape the `AccountCredentialStore` port's
  `Result` exists to remove: a report that appears only when nothing went wrong is not a report.
  The lines are built by `credential_log.rs` as pure functions, so what they say (and what they
  keep out) is unit-tested without a logger, and `mailcal-bindings/tests/credential_logging.rs`
  proves they reach the log.
- **An account is named by a handle, never by its id.** `account[0]` is a position in one
  particular list (the stored-config order at boot, the dial order on a reconnect) and works
  only where that list is in scope. A token refresh has no list, so those lines went unattributed:
  three identical rotation lines in one launch read the same whether that was three accounts
  rotating once or one account rotating three times, which are a healthy launch and a refresh
  loop. `mailcal_account::account_log_handle` gives a short, stable, non-reversible handle
  (`acct:1a2b`) for exactly this. Never invent a *second* numbering: a label that means different
  accounts on different lines costs more than no label at all.
- **Privacy is a core guarantee** (`logging.rs` module docs): the core logs counts, durations,
  ids, and high-level events, **never** mail/event content, addresses, or credentials. The stream
  is therefore safe to surface to a user and to attach to a support report.
  An **account id is an address and a host**, so it is never logged: that is what the handle
  above is for. (Whether this binds the *clients*, which write into the same file, is an open
  gap: the Windows client logs the address today and Apple/Android log nothing.)
  **Contacts fall under exactly this rule**, and are the most identifying thing the app holds: the
  contacts paths log row counts, source counts and durations, never a name, email address, phone
  number or organisation. An **address-book id** is a container id, not content, and may be logged
  (a skipped shared book is undiagnosable otherwise); the cards inside it may not. A search term
  and a composer autosuggest token are themselves a name and an address, so those paths log the
  **character count** and never the text. This half is machine-checked:
  `crates/mailcal-app/src/tests_contacts_logging.rs` drives every contacts path against a
  capturing logger and fails if a card's own values reach a log line.
- **Every contacts stage announces itself**, because the log is the only instrument a live
  contacts session has. Binding logs how many sources came up per account and names each silent
  skip (no CalDAV endpoint · no JMAP contacts capability · discovery failed · timed out); connect
  adds a `contacts_source[n] book=… writable=…` line per bound source, and an explicit
  `contacts_sources=0` for the account families that bind none; sync logs per-source applied
  counts, an `unavailable` source's server-given reason, and a per-pass summary; the reads log
  row/match counts with durations. The point is that "Contacts is empty" resolves to the stage
  that produced nothing instead of to a guess. Same test file gates that the lines exist.

## The shared bar

Each client implements `Logger` over a **native rotating file sink**. Every sink meets:

- **Size-based rotation, ~4 MB cap.** `1 MB` per file × `3` backups: at the cap,
  `<log>` → `<log>.1` → … → `<log>.3` (oldest dropped), then a fresh `<log>`. Never unbounded.
  The check is **pre-write** (the file may exceed 1 MB by one line before it rolls); each rotation
  destination is vacated before its move, so a plain rename never collides.
- **INFO by default.** Keeps a long, useful window within the cap. `DEBUG`/`TRACE` is **opt-in**
  (see matrix) for a support session, not the default.
- **Best-effort.** A log that can't open, rotate, or write **stays silent**: it must never break
  startup or crash the app. Every path swallows IO errors.
- **Thread-safe.** The core calls the logger from runtime worker threads, so each sink serializes
  its rotation check + write (serial queue / lock).
- **A crash says so on the way out.** An unhandled exception is written to the log (its type, its
  message and its stack) before the process dies. Without it a crash is indistinguishable in the
  file from a clean exit: the log simply stops, with nothing wrong on the last line. That is not
  hypothetical. A `COMException` out of `NavigationView.IsPaneOpen` took the Windows client down as
  a stowed exception inside `Microsoft.UI.Xaml.dll`, and the only record of it anywhere on the
  machine was a Windows Error Reporting entry naming an offset in `combase.dll`; the log a user
  would have attached to the report said nothing at all. A stack is frames and type names, so the
  never-log-content rule below is untouched, and the handler must **not** mark the exception
  handled: the process is still meant to fail, this only makes it explain itself first.
- **A Rust panic is a crash too, and it is the one no client can see for itself.** The panic hook in
  `crates/mailcal-bindings/src/crash.rs` writes the payload, the file and line, the thread and the
  stack, through the same `Logger` every other record uses, so it lands in the same file on all
  four platforms at once. It is not an optimisation of the host handlers, it is the only record
  there is: a panic on a runtime worker thread kills that task and reaches no host handler at all,
  and one under a host-initiated call is caught at the FFI boundary and handed over as a **message
  alone**. The stack the host then reports is the host's own, starting at the generated binding.
  The two are joined in the *file* (same log, moments apart, same payload text), never in one
  trace, because the two runtimes unwind on separate machinery. The first three panics of a run
  carry a stack; after that the headline still reports each repeat, so a watcher panicking on every
  reconnect cannot fill the rotating cap with identical frames and evict what explains the first.
- **A native fault says so too, and it is the one that reaches no handler above it.** A SIGSEGV
  inside the cdylib, or an `abort()` under it, is neither a Rust panic nor a host exception: the
  panic hook does not run, the host's uncaught-handler does not run, and the log stops mid-line.
  `watch_for_native_faults` (`crates/mailcal-bindings/src/native_fault.rs`) writes one banner
  naming the signal and the **faulting address**: `0x0` says a null dereference, anything else
  says the pointer had been somewhere. Each platform then adds what it can afford to: Linux the
  frames, Windows the **module and the offset into it**, which is the half of an address that
  survives the machine it was written on. Three rules bind it on every platform that arms it:
  - **The record is prepared at install, not at fault.** A signal handler may not allocate, format
    or take a lock, so the path and one opening per signal are built while that is still legal and
    the handler only ever writes bytes that already exist.
  - **The platform's own reporting is left completely alone.** On POSIX the displaced handler is
    restored and re-raised, never replaced with `SIG_DFL`: something is already installed for these
    signals: the Rust runtime reports a stack overflow from one, and a platform crash reporter
    sits under that, so dropping the chain would trade a full crash report for one line in a file.
    Chaining leaves the tombstone, the core dump and the store console report exactly as they were.
    (Apple is the one client that may use `SIG_DFL`, because Darwin reports through Mach exceptions
    rather than the signal disposition.) On Windows the same rule takes a different form: the
    vectored handler returns `EXCEPTION_CONTINUE_SEARCH` on every path, so it only ever observes.
  - **The ordinary log stands down while a record is being written**, the same rule and the same
    reason as Apple's: two writers on one file, no shared lock, and half a stack under someone
    else's line is worse than none. The core's own stream is held back inside the log bridge, so
    every platform gets it from one place.
- **Session marker, and it names the build.** Each sink stamps a
  `--- session start (<app version> [build <id>], <device/os>) ---` line on init, so a session
  boundary is visible in a rotated/merged log **and the log can be pinned to a build**. The version
  is required because [`/VERSION`](versioning.md) holds the last *released* version: a dev build and
  the shipped one report the same marketing version. Store build numbers identify Apple, Android
  and Windows packages; Linux uses a source fingerprint plus build epoch because Flatpak has no
  build number. A log with no such identifier is a support artefact nobody can act on. A version is
  not content, so the never-log-content rule below is untouched.

## A log line describes the user's mail, never our source tree

The log is a file the user can open, read, and attach to a support request. That makes every line
**product surface**, held to the same bar as any other copy we ship, so a line says *what happened*
and *what it means for them*, and never names how we are built.

Concretely, a line must not contain:

- a **path in this repository** (`docs/provider-oauth.md`, `crates/…`) or a rule number in a design
  doc;
- an **issue or PR number** (`#1234`);
- an **internal identifier**: a module, type, struct field, or function name (`the registry`,
  `StoredOutcome`, `re-serialize`, `view-model`). A *protocol* term is fine and often necessary
  (`invalid_grant`, `IDLE`, `CONDSTORE`, `JMAP`): those name the thing the server said, which is the
  fact being reported.

The reason is not tidiness. An internal reference is a promise the log cannot keep: it is
meaningless to the person holding the phone, it is **stale the moment the code moves**: a doc path
in a shipped binary cannot be renamed by a refactor, and it reframes a report about the user's mail
as the app talking about itself. Our own reasoning belongs in a comment beside the code, where a
reviewer and a compiler keep it honest.

Two consequences worth stating, because both were got wrong:

- **Severity follows the outcome, not the obscurity of the cause.** A failure that will cost the user
  their sign-in at the next restart is an `error!` whether the cause is a locked keychain or an
  encoding step no user has ever hit.
- **Say the consequence, not the internal state.** *"the stored credential stays one generation
  behind until the next rotation"* named our data model and described a stale cache; what it actually
  meant was *"this account will ask to be signed in again after the next restart"*. A support log sat
  for two days carrying that line, and nobody read consequence into it, including its author.

**The one exception is a crash record, and it is an exception on purpose.** A panic's location and
its stack *are* our source tree: that is the whole content of the report. The rule above exists
because an internal reference is meaningless to the reader and stale the moment the code moves; a
crash location is neither. It is generated by the build that is running, so it cannot drift from the
code, and its reader is not the user but the engineer they hand the file to. Without the file and
the line the log says "it crashed" and nothing more, which is the silence this feature exists to
remove. Nothing else earns this: a *hand-written* line citing a path or a doc stays forbidden, and
`check_log_hygiene.py` still fails one, because it inspects string literals and a crash location is
never a literal.

The narrow, unambiguous half of this is machine-enforced by
[`check_log_hygiene.py`](../scripts/ci/check_log_hygiene.py) (in the gate): a repo path, a `.md`
reference, an `#nnn`, or a raw Rust `{account_id}` / `{account}` interpolation inside a log macro's
string fails the build. The judgement half, jargon, stays with the author and the reviewer,
because a checker that guesses at prose cries wolf and gets skipped.

## Per-platform implementation matrix

| Aspect | Apple · `FileLog.swift` | Windows · `Services/Log.cs` | Android · `FileLog.kt` | Linux · `logger.rs` |
|---|---|---|---|---|
| Sink file | macOS: `~/.local/share/mailcal/mailcal.log` (+ `.1..3`); iOS/iPadOS: app Application Support `mailcal/mailcal.log` (+ `.1..3`) | `%LOCALAPPDATA%\Allodia\MailCalendar\logs\app.log` (+ `.1..3`) | `<filesDir>/logs/app.log` (+ `.1..3`) | `$XDG_DATA_HOME/mailcal/mailcal.log` (fallback `~/.local/share`; + `.1..3`) |
| Line format | `<ts> <LEVEL> [target] msg` (local, ms) | `<ts ±offset> [level] msg` (core lines carry `[target]` in msg) | `<ts> <LEVEL> [target] msg` (local, ms) | `<ts ±offset> <LEVEL> [target] msg` (local, ms) |
| Rotation | 1 MB × 3 backups (~4 MB), pre-write | 1 MB × 3 backups (~4 MB), pre-write | 1 MB × 3 backups (~4 MB), pre-write | ✅ 1 MB × 3 backups (~4 MB), pre-write |
| Default level | `INFO` unless the Diagnostics toggle is on (`DiagnosticsPrefs.swift`) | `Info` via `DiagnosticsLog.ResolveLevel` (`MailboxModel.Accounts.cs`) | `INFO` unless the Diagnostics toggle is on (`DiagnosticsPrefs.kt`) | ✅ `INFO` unless the host preference is on |
| DEBUG opt-in | Diagnostics toggle (persisted) | `ALLODIA_LOG_LEVEL` env **or** Diagnostics toggle (env wins) | Diagnostics toggle (persisted) | ✅ Diagnostics toggle (persisted) |
| Init point | `FileLog.shared` (lazy; marker on init) | `Log.Init(AppPaths.Root)` in `Program.Main`, before `Application.Start` | `FileLog.init(filesDir)` in `onCreate`, before `connect` | ✅ `FileLogger::new` before `new_accounts` / `new_demo` |
| Version in the session marker | ✅ `CFBundleShortVersionString` + `CFBundleVersion` (dotted UTC timestamp per package) | ✅ assembly `<Version>` + MSIX `Package.Current.Id.Version` when packaged (unpackaged dev loop falls back to the assembly version) | ✅ `BuildConfig.VERSION_NAME` + `VERSION_CODE` | ✅ `CARGO_PKG_VERSION` + source fingerprint/build epoch |
| Thread-safety | serial `DispatchQueue` | `lock (Gate)` | `synchronized(lock)` | ✅ `Mutex` around rotate + write |
| Shared `ConnectionInfo` records | Captured through `Logger` | Captured through `Logger` | Captured through `Logger` | ✅ Captured through `Logger` |
| Shared connect-step trace (URL-free) | Captured through `Logger` | Captured through `Logger` | Captured through `Logger` | ✅ Captured through `Logger` |
| Unhandled-exception capture · **host runtime** | ✅ `CrashLog.swift`: `NSSetUncaughtExceptionHandler` (armed a run-loop turn late) + a `sigaction`-free signal handler for ABRT/BUS/FPE/ILL/SEGV/TRAP, each re-raised so the OS reporter still fires | ✅ `Services/CrashLog.cs`: XAML `UnhandledException` + `AppDomain.UnhandledException` + `TaskScheduler.UnobservedTaskException`, none marked handled | ✅ `CrashLog.watchForCrashes`: `Thread.setDefaultUncaughtExceptionHandler`, chained to the handler it replaced | ✅ `crash.rs`: `glib::log_set_default_handler` for GLib warning/critical/error, chained to the default handler |
| Unhandled-exception capture · **Rust panic** | ✅ shared hook (`crash.rs`) | ✅ shared hook (`crash.rs`) | ✅ shared hook (`crash.rs`) | ✅ shared hook (`crash.rs`) |
| Previous-session capture · **taken away, not faulted** (OOM / watchdog kill / hang) | ✅ `CrashDiagnostics.swift`: MetricKit `MXCrashDiagnostic` + `MXHangDiagnostic`, headline only, reported at the next launch and naming the build that died | ❌ none: Windows Error Reporting holds it | ❌ none: an ANR reaches Play Console, not the log | ❌ none: the journal and systemd-coredump hold it |
| Unhandled-exception capture · **native fault** | ✅ the client's own Swift handler (row above): signal **and** frames, which it can symbolize in-process | ✅ shared `watch_for_native_faults`, armed in `Program.Main`: a **vectored** exception handler (a fault here is not a signal), filtered to this DLL so .NET's own access violations are not reported, never handling (WER still fires), and naming the module and offset the address alone cannot be resolved from | ✅ shared `watch_for_native_faults`, armed in `onCreate`: banner and address only, no frames: bionic's `backtrace(3)` is `__INTRODUCED_IN(33)` and `minSdk` is 31 | ✅ shared `watch_for_native_faults`, armed beside the GLib handler: banner, address **and** frames via `backtrace_symbols_fd` |
| Also to platform logger | `os_log` (Console.app / device logs) | (file only) | Logcat (`adb logcat -s Mailcal`) | (file only) |

## The Diagnostics surface (Settings → Diagnostics)

Every shipping client has a **Settings → Diagnostics** surface over its own sink: the in-app "hand us
the log" feature this contract exists to enable. The surface is the same everywhere:

- **View.** The **current** log file, read-only and monospace, newest last; the viewer opens at
  the end and offers a jump-to-end affordance. Reads are best-effort, tolerate a concurrent
  writer, and never break the app; same discipline as the sink itself.
- **Share / export.** Hands the **current file only** to the system share sheet
  (Android / iOS / macOS) or a save-file dialog (Windows); backups stay on the device. The
  privacy note (what the file does and does not contain) is surfaced **before** the file
  leaves the device (a confirm step, or on macOS a note directly beside the share control,
  since `ShareLink` offers no confirm hook).
- **Size / rotation state + copy path.** Total bytes across current + backups (SI units on every
  platform), the backup count, the ~4 MB cap note, and one-tap copy of the absolute log path.
- **"Include more detail" (DEBUG) toggle.** ON calls `MailcalApp::set_log_level(Debug)` on the
  live core; OFF returns to `Info`. The choice is **persisted client-side** and fed to every
  core-construction site as the boot `log_level` (background workers included), so a support
  session survives a relaunch. The default stays `INFO`; the toggle is the user-facing opt-in
  the matrix above documents.

| Aspect | Apple | Windows | Android | Linux |
|---|---|---|---|---|
| Entry point | Settings category (macOS) / section (iOS), after Advanced | Settings category, after Advanced | Settings section, after Advanced → own screen | ✅ Settings category, after Advanced |
| Share / export | `ShareLink` (macOS); confirm → `UIActivityViewController` (iOS) | inline confirm → `FileSavePicker` | confirm → `ACTION_SEND` (FileProvider over `files/logs/`) | ✅ privacy confirm → native file chooser |
| Persisted toggle | `UserDefaults` `diagnostic_log_debug_enabled` | `<dataDir>\loglevel.txt` (`debug`/`info`) | SharedPreferences `diagnostic_log_debug_enabled` | ✅ XDG host preferences `diagnostics_debug` |
| Boot sites honouring it | `newAccounts`, `newBackgroundWorker` (cold path), `newDemo`, `newShowcase` | `ConnectAsync`, `ConnectShowcaseAsync` (env wins) | `newAccounts`, `newShowcase`, `newBackgroundWorker` | ✅ production accounts, demo, and harness fixtures |
| Implementation | `DiagnosticsSettings.swift` (+ viewer, prefs) | `SettingsDialog.Diagnostics.cs` + `DiagnosticsLog.cs` | `DiagnosticsScreen.kt` + `DiagnosticsPrefs.kt` | ✅ `ui/settings/diagnostics.rs` + `preferences.rs` |

## Known gaps / follow-ups

- **On Apple the signal handler writes no timestamp**, because it cannot: formatting one needs
  allocation, and a signal handler may not allocate. The record is appended straight after the last
  timestamped line, so its position dates it to within that line, and its `*** … ***` banner makes
  it obvious at a glance that the block is not an ordinary entry.
- **On Apple, ordinary log writes stop once a crash record starts**, and they have to. A signal
  handler cannot take `FileLog`'s serial queue, so it writes on a descriptor of its own: two
  writers on one file with no shared lock. Measured on an iPhone simulator: two `DEBUG` lines from
  a worker thread landed between frames 3 and 4 of a SIGABRT stack, which is the "half a stack
  under someone else's line" the one-record rule above exists to prevent. The handler now claims
  the file with a flag the ordinary path checks before every write. The residual: a write already
  in flight when the signal arrives can still land inside the record; nothing after it can.
- **MetricKit reports the previous session, and only the headline.** `CrashDiagnostics.swift`
  subscribes for `MXCrashDiagnostic` and `MXHangDiagnostic`, which is how the three deaths the
  live handler cannot narrate get into the log at all: a memory-pressure (jetsam) kill, a watchdog
  kill, and a hang: for the first two no signal is delivered, and for the third nothing crashed.
  Three limits, each deliberate:
  - **It is retrospective, so it does not spend the word `unhandled`.** These lines are about a
    session that ended, possibly days ago; `unhandled` means "the process is dying now" on four
    clients at once, and one grep must not return both kinds of thing.
  - **The call-stack tree is not written.** It is unsymbolicated binary offsets, large enough to
    evict the history it was appended to from the rotating cap, and for a jetsam or watchdog kill
    there is no meaningful stack anyway. The OS crash report holds it.
  - **Delivery is best-effort and untested end to end.** Only the system produces a payload, at a
    launch after the one that died, so no gate can drive it; the wording and the signal-name
    mapping are unit-tested and the delivery is not. macOS support is declared from 12.0 but has
    not been observed here; verify by leaving a crashed build to be relaunched, on a device.
- **An Apple exception handler armed too early is silently discarded.** AppKit replaces the
  uncaught-exception handler while `NSApplication` sets itself up, which is *after* `App.init()`
  runs, so an install made there never fires, and `NSGetUncaughtExceptionHandler()` still returns
  yours, so nothing looks wrong. `CrashLog` arms it a main-queue turn later for exactly this
  reason. Measured on macOS 26: an `NSException` on a background thread reached SIGABRT with no
  line of its own from the early install, and wrote one from the late install.
- **An `NSException` on macOS's main run loop is not a crash at all**: AppKit catches it and the
  app carries on, so there is nothing for a crash handler to say. The shapes that do kill this app
  are an exception off the main queue (libdispatch's callout aborts on one), a Swift trap, and a
  native fault. `MAILCAL_CRASH_TEST` triggers on a background thread for that reason.
- **A handler runs only if the sink is already open.** Every one of these writes through its
  client's file log, and each drops everything silently until that log has a path, so the arming
  goes *after* the init, in the same breath. Android arms its handler in the line after
  `FileLog.init`, itself the first statement of `onCreate`; Windows opens the log and arms its
  handlers in `Program.Main` before `Application.Start`, which is what puts XAML init and early
  startup inside the covered window rather than outside it.
- **Linux is uncovered until the core boots, and that is a decision, not an oversight.** The sink
  and the panic hook both arrive with the core (`boot::app`), so a panic or a GTK critical raised
  earlier (the preferences read, the locale override, GTK's own start-up, the window root)
  reaches the file nowhere. Hoisting the sink into a global to close it was rejected because the
  window it would cover is exactly the window in which there is no window: the user cannot reach
  Settings → Diagnostics to hand the file over, and a developer with shell access already has
  stderr and the journal, where the default GLib handler and the default panic hook both still
  print. The reasoning lives beside the code in `clients/linux/src/boot.rs`.
- **Two .NET faults still never reach any handler**, whatever the ordering:
  `StackOverflowException` and `Environment.FailFast`. The CLR tears the process down without
  raising the event, and neither is reachable from below it either: `FailFast` is *defined* not to
  run handlers, and a managed stack overflow is failed fast rather than raised as the hardware
  fault a vectored handler would see. Windows Error Reporting is the only record of either.
  The third of the old trio, **a native access violation inside the cdylib, is now covered**: the
  vectored handler above sees it before the CLR does.
- **A shipped Android stack names functions but not lines, and that is the deal we chose.**
  Measured deflated, which is what a device downloads: bare addresses **10.4 MiB**, names
  **11.9 MiB**, names plus file and line **24.2 MiB**, against **40.6 MiB** for what shipped
  before this, which carried line tables for every dependency as well. Names are what makes a
  support log readable; the rest more than doubles the native download for a precision nobody
  reading one has asked for.

  **Why this is not left to AGP, which is the obvious question.** There is exactly one native
  artifact: `libmailcal_bindings.so`, holding the whole Rust core, the engine, every Rust
  dependency and the `extern "C"` UniFFI exports. (The Kotlin bindings UniFFI generates are DEX,
  and R8 handles those; native stripping never touches them.) Stripping is per-file, so any AGP
  setting applies to all of it at once, and AGP offers only two outcomes:

  | | shipped, deflated | nameable functions in the log |
  |---|---|---|
  | AGP strips (`--strip-unneeded`) | 10.4 MiB | **438** (`.dynsym`, the FFI exports the loader needs) |
  | AGP exempts (`keepDebugSymbols`) | 24.2 MiB | 38,219, with file and line |
  | ours (no DWARF emitted) | **11.9 MiB** | **38,221** |

  AGP's flag is hardcoded `--strip-unneeded`, which takes `.symtab` with the DWARF, and
  `keepDebugSymbols` means "do not strip this file at all"; it is checked before AGP even looks
  for a strip tool. There is no setting for the middle. Measured on a device, the difference is
  total: a Rust backtrace resolves every frame with `.symtab` present and reads `<unknown>` at
  every frame without it.

  So `ndkVersion` is set and AGP strips everything it should, `keepDebugSymbols` exempts **only**
  our cdylib, and `build-release.sh` builds that one with no DWARF at all
  (`CARGO_PROFILE_RELEASE_DEBUG=0`), so nothing has to be stripped afterwards. The glob names the
  library deliberately: widened to `**/*.so` it would exempt JNA and androidx too, and then
  `ndkVersion` would buy nothing.

  **Why not upload symbols to Play instead and ship none.** `ndk { debugSymbolLevel }` does work
  (verified: a `SYMBOL_TABLE` build put a 37 MB `.sym` in the bundle's metadata while the packaged
  library stayed stripped), and it would give the smallest download. It answers a different
  question. Play symbolicates **tombstones**; this log symbolicates **in-process**, against the
  loaded library's own symbol table. And the shape a Rust core fails in most does not produce a
  tombstone at all: the release profile sets no `panic = "abort"`, so a panic unwinds and the FFI
  boundary catches it, leaving this log as the only record. Shipping no symbols is possible, but
  only by changing what the log records: module plus offset per frame, resolved offline against a
  retained build. That trades away the thing this document exists for (a file a user hands over
  that a human can read) to save 1.3 MiB, so it is not the deal we took.

  ⚠️ **The outcome is asserted on the artifact, because none of the above can fail loudly.**
  `StripDebugSymbolsTask` copies its input straight through when the strip tool is missing, when
  the strip exits non-zero, and when the file matches `keepDebugSymbols`, logging the first two at
  `verbose`. There is no setting that turns a silent pass-through into a build failure. That is how
  this went unnoticed for so long: nothing was stripping, for two independent reasons at once (no
  `ndkVersion`, and a `keepDebugSymbols` glob covering everything), and every build reported
  success. `scripts/dev/check-android-native-libs.sh` now reads the packaged `.so` and fails if our
  cdylib has lost its `.symtab` or kept a `.debug_*` section.
- **A shipped Apple stack names functions but not lines, on the same deal as Android.**
  `STRIP_STYLE: debugging` in `clients/apple/project.yml` keeps the symbol table in the binary.
  Xcode's default for an application is `all`, which was what shipped: it leaves 2,282 symbols of
  140,259, and every frame of `backtrace_symbols_fd` (the crash handler's only symbolizer) reads
  `AllodiaMail + 1852`. Measured on the macOS Release binary, gzipped as a stand-in for what the
  store delivers, the symbol table costs **15.6 → 17.9 MiB**.

  No file and line either way, and that is not a choice: `dwarf-with-dsym` has `dsymutil` move the
  DWARF into the `.dSYM` *before* the strip runs, so the shipped executable never carries it. The
  `.dSYM` goes to App Store Connect, which symbolicates Apple's own crash reports: the same split
  as Play's, and with the same hole in it, since a Rust panic unwinds into the FFI boundary and
  never becomes a crash report at all.

  ⚠️ **Not `STRIP_STYLE: non-global`**, which reads like the cheaper middle (20,405 symbols for
  2 MB rather than 22 MB) and is worse than shipping none. `dladdr` reports the nearest
  *preceding* symbol, so once local symbols are gone a frame resolves to whichever global happens
  to sit below it. Measured with a probe: a `fileprivate` frame came back confidently labelled as
  the public function two frames above it. Nothing in the output says it happened, and a support
  log that names the wrong function costs more than one that names none.

  ⚠️ **The handler's own topmost frame can still misname itself, and that is expected.** The
  signal handler is a C function pointer built from a Swift closure, and that thunk carries no
  symbol, so `dladdr` falls back to the nearest preceding one and frame 0 reads something like
  `block_destroy_helper + 1520`. Every frame below it is correct. Read frame 0 as "inside the
  handler", never as the function it names.

  Verified on a Release build with `DEPLOYMENT_POSTPROCESSING=YES` (so `strip` genuinely ran)
  and with the `.dSYM` deleted first, or the system would have symbolicated from it and proved
  nothing: a real `SIGSEGV` produced a crash report with **95 frames in our binary, 95
  symbolicated, 0 bare offsets**, naming `main` and the `tokio` runtime's own frames.
  `Scripts/package.sh` asserts the symbol count at both pre-upload gates, because `STRIP_STYLE`
  reverting breaks nothing else: the app builds, runs and signs identically, and only a crash
- **The Windows handler is filtered to this DLL, and that filter is load-bearing.** A vectored
  exception handler is called for **every** exception in the process, and .NET raises them as
  ordinary control flow: a managed `throw` is one, and a *caught* `NullReferenceException` is a
  hardware access violation in JIT-ed code. So the handler declines anything whose faulting
  instruction is not inside `mailcal_bindings.dll` (`GetModuleInformation` at install). Without
  that check the log fills with reports of the app working correctly. It also never handles
  anything: every path returns `EXCEPTION_CONTINUE_SEARCH`, so the CLR reacts exactly as it
  would have and Error Reporting still fires.
- **The Windows record names a module and an offset, because the address beside them is worth
  nothing off the machine that wrote it.** Windows randomizes an image's base at every boot, and
  debug info is indexed by offsets rather than addresses, so an absolute address in a handed-over
  log names a different byte on the machine that reads it, and resolves against nothing. The
  record carries both: `… an access violation at 0x7ffaff810010 (mailcal_bindings.DLL+0x10) …`.
  The name is spelled the way the loader recorded it, which is why the extension is upper case
  there and lower case on disk; Error Reporting spells it the same way. The offset is what
  `llvm-symbolizer --obj=mailcal_bindings.dll --relative-address 0x59a24` turns back into a
  function and a file and line, months later, against the build that died. Keeping the absolute
  address as well is not redundancy: the pair gives up the load base, which is what lines the
  record up with the module list in Error Reporting's own entry for the same fault: Windows files
  those two fields under the names `Faulting module name` and `Fault offset`, so a handover
  already speaks them.
  ⚠️ **The PDB has to be found, not shipped.** `--obj` takes the **DLL**, and the symbolizer looks
  for the PDB beside it, but `Mailcal.csproj` copies the cdylib into the app alone, so the app's
  own directory resolves nothing. Point it at the pair cargo left in
  `target/<triple>/<profile>/`, from the commit that built the app that died.
- **The Windows handler is only ever exercised on a Windows host.** `native_fault_windows` is
  `#[cfg(windows)]`, so the test that drives a real fault all the way through it (arm a child
  process, dereference null inside it, read its log back) exists nowhere else, and a macOS or
  Linux gate reports *no tests at all* rather than a skip. Only the record's wording is covered
  everywhere (`native_fault_record`). Run `cargo test -p mailcal-bindings native_fault` on Windows
  before trusting a change to it.
- **The record is appended with `FILE_APPEND_DATA` alone, and the mask is load-bearing.** Win32
  appends without a seek only while `FILE_WRITE_DATA` is *withheld*: granted both, the handle
  honours its file pointer, which `OPEN_ALWAYS` leaves at zero, and the record is written **over**
  the head of the log, taking the session marker that names the build that died, which is the one
  line a handover needs most. It reads as a working feature, because the record itself is there
  and correct.
- **Android's fault record carries no frames**, only the signal and the faulting address. Bionic
  gained `backtrace(3)` and `backtrace_symbols_fd(3)` at API 33 (`__INTRODUCED_IN(33)` in the NDK's
  `execinfo.h`) and this client's `minSdk` is **31**. It is not the loss it looks like: Android is
  the one platform whose own reporting is *better* than ours, and the record's job there is to mark
  the log with the moment the process died so it lines up with the tombstone. **Nor does it get
  the module and offset Windows writes**: POSIX reports `si_addr`, which is the address the
  faulting instruction went *for* rather than the instruction itself, usually in no module of
  ours, and never the number a symbolizer wants. Raising `minSdk` to 33 is what changes it, and until then the line holds itself, because the NDK's stub `libc.so`
  carries those symbols only from API 33 up, so widening the `cfg` fails to link rather than
  shipping something that would not load on Android 12.
- **No gate runs on a device unless someone runs it.** A real SIGSEGV in a forked child drives the
  install, the chain and the write (`native_fault.rs`, `native_fault_faulting.rs`); the frames are
  asserted on Linux and the banner-alone record on Android, each under the `cfg` that decides it.
  Linux is covered by every Rust change, because the workspace test job runs there, and Apple by
  the `cfg(test)` build of `armed` on a dev machine. Android needs
  [`test-android-native-fault.sh`](../scripts/dev/test-android-native-fault.sh) and an attached
  device or emulator; no CI job runs it, so a change to the handler is where to remember it.

  ⚠️ **The helpers those two arms share are themselves `cfg(any(linux, android))`**, so a macOS
  run compiles `native_fault_faulting.rs` and not them. A dev machine therefore type-checks the
  file while leaving the half that reads a record unbuilt: rename something in
  `native_fault_record` and both Linux and Android break with a green local gate. Only the Linux
  job and the script above see it.
- **No gate exercises the *packaged* half of the Windows session marker.** The composition of both
  shapes is unit-tested (`SessionMarkerTests`) and the running app's line is asserted end-to-end by
  `clients/windows/uitests/SessionLog.Tests.ps1`, but every gate runs the app **unpackaged**, where
  `AppIdentity.PackageVersion` is `null` and the marker names the marketing version alone. The
  `Package.Current.Id.Version` read itself executes only inside a real MSIX, the same shortfall
  [`versioning.md`](versioning.md) already records for the manifest version stamp. **Verified by
  hand instead**. Repeat this whenever the packaging or the marker changes: build with
  `clients/windows/package.ps1 -Sign`, install the bundle with `Add-AppxPackage`, launch, and read
  the newest session line. On 2026-08-01 (Windows 11 arm64) that gave
  `--- session start (0.2.2 package 0.2.9709.16017, Arm64, Microsoft Windows 10.0.26200) ---`, whose
  second number matches `Get-AppxPackage`'s `Version` exactly, and it landed in the **unvirtualized**
  `%LOCALAPPDATA%\Allodia\MailCalendar`, so a packaged and an unpackaged run share one file and one
  copy path rather than the log splitting into a per-package `LocalCache`. The read fails soft by
  construction (an unexpected failure leaves the assembly version in place), so the cost of the gap
  is a support log that names one version instead of two, never a failed launch.
- **Android ad-hoc native warnings are Logcat-only.** The **core stream** (the bulk and most
  diagnostic content) is captured to the file on Android, matching Apple. A handful of host-side
  warnings logged directly via `android.util.Log.w/e(TAG, …)` in `MainActivity.kt` /
  `MainActivityCore.kt` (composer/attachment/OAuth failures) still go only to Logcat. Follow-up:
  route those through `FileLog` too so the attachable file is complete.
- **No env-var level override on Apple/Android.** Only Windows reads `ALLODIA_LOG_LEVEL`. The
  user-facing opt-in is now the persisted Diagnostics toggle on every platform; a parallel
  env/debug-build override on Apple/Android remains a possible dev convenience, nothing more.
- **Share/export hands over the current file only.** Backups (`.1..3`) stay on the device, so a
  support request filed just after a rotation carries a short window. If support regularly needs
  the full ~4 MB history, bundling the backups into the share is the follow-up.
- **A pre-rotation oversized log is never truncated once parked as a backup.** Rotation caps the
  *live* file; a log that grew past the cap before rotation shipped is moved to `.1` whole and
  stays there until it ages out. "Log size" in Diagnostics reports it honestly, so the number can
  legitimately exceed the ~4 MB cap note on old installs.
- **Windows export failure is silent.** A failed `FileSavePicker` export deletes the half-written
  file and logs the exception type, but there is no localised user-visible error message yet.
- **Auto-attach to a support ticket isn't built.** There is no support backend; Diagnostics
  view/share is the manual flow that unblocks it.
- **On a release build, Settings → Diagnostics → Share is the supported way to get the log**:
  the cable routes are closed by design (`run-as` refuses a non-debuggable package; since
  **Android 12** `adb backup` excludes a non-debuggable app's data). Keep the fallback for the
  one case share can't cover (an app that won't boot far enough to open Settings): build the
  `release` type with `isDebuggable = true` **signed with the key the installed APK actually
  carries** (check it: `apksigner verify --print-certs` on `adb pull $(adb shell pm path <pkg>)`;
  a locally-built "release" is often signed with the **Android debug key**, not the upload key),
  then `adb install -r` (replaces in place, data preserved) and `run-as … cat files/logs/app.log`.
  A signature mismatch fails *safely*; never "fix" it with `adb uninstall`, which **destroys the
  user's accounts and mail store**. Restore the non-debuggable build afterwards.

## Enforcement

This contract is binding via [`../AGENTS.md`](../AGENTS.md). When you change diagnostic logging:

1. Update this document (the rule **and** the matrix) in the same change.
2. Keep the policy identical across every existing platform: cap, default level, privacy,
   best-effort, thread-safety.
3. A new platform ships a rotating, capped, privacy-safe log **before** it ships to users; any
   shortfall goes under "Known gaps" with a follow-up, never left silent.
