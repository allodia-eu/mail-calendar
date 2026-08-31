// The uncaught-exception handler: what the diagnostic log says when the JVM half of the app dies.
//
// Without it a crash is indistinguishable in the file from a clean exit, the log simply stops,
// with nothing wrong on the last line (docs/logging.md → "A crash says so on the way out"). This
// covers every JVM thread in the process, WorkManager workers included.
//
// It does NOT cover the two native cases, which have handlers of their own in the core: a Rust
// panic writes through the shared hook (crates/mailcal-bindings/src/crash.rs) and a SIGSEGV or an
// abort inside the cdylib through the native-fault handler `watchForNativeFaults` arms below.
package eu.allodia.mailcal

object CrashLog {
    // Arms the handler. Called from MailcalApplication.onCreate immediately after FileLog.init, so
    // it is in place on both process entry points, a user launch and a cold WorkManager wake.
    fun watchForCrashes() {
        Thread.setDefaultUncaughtExceptionHandler(
            handler(Thread.getDefaultUncaughtExceptionHandler()),
        )
    }

    // The handler itself, over the handler it replaces.
    //
    // **Chaining is not politeness.** Android's own default handler is what shows the crash dialog,
    // reports to Play Console, and kills the process; swallowing it leaves a dead app that never
    // says it died. So the record is written first and `previous` runs after, FileLog.append is
    // synchronous under its lock, so the line is on disk before the process goes.
    //
    // Taking `previous` as a parameter rather than reading it here keeps the global handler out of
    // the test JVM: the wiring above is the only thing that mutates process state.
    internal fun handler(previous: Thread.UncaughtExceptionHandler?): Thread.UncaughtExceptionHandler =
        Thread.UncaughtExceptionHandler { thread, throwable ->
            FileLog.append("ERROR", "crash", record(thread.name, throwable))
            previous?.uncaughtException(thread, throwable)
        }

    // The record. `unhandled` is the word every platform's crash line carries, one string support
    // greps across four clients, and the stack's own first line already reads `<type>: <message>`,
    // so the headline names the thread and the frames say the rest without repeating it.
    internal fun record(thread: String, throwable: Throwable): String =
        "unhandled on $thread: ${throwable.stackTraceToString().trimEnd()}"

    // Arms the core's native-fault handler over the same file. A SIGSEGV or an abort inside the
    // cdylib is neither a JVM throwable nor a Rust panic, so the handler above never sees it and
    // the log just stops; this writes one banner naming the signal and the faulting address. The
    // handler chains to Android's own, so the tombstone and the Play Console report are unchanged.
    //
    // It takes the path rather than being folded into `watchForCrashes` because this is the one
    // call here that crosses the FFI, and `:app:test` loads no cdylib, keeping them apart is what
    // lets the handler above stay unit-tested. A missing library is swallowed for the same reason
    // every other log failure is: diagnostics must never be what stops the app from starting, and
    // a cdylib that is genuinely absent takes the app down at its first real call regardless.
    fun watchForNativeFaults(logPath: String) {
        try {
            uniffi.mailcal_bindings.watchForNativeFaults(logPath)
        } catch (_: Throwable) {
        }
    }
}
