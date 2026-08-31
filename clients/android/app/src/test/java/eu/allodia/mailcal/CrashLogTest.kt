// The uncaught-exception handler: what it writes, and that it still lets the app die.
//
// Plain JUnit, no Robolectric and no Android classes. `record` is pure text, and `handler` takes
// the handler it replaces as a parameter precisely so a test can drive it without touching the
// test JVM's process-global default handler.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CrashLogTest {
    @Test
    fun the_record_leads_with_the_word_every_platform_greps() {
        val record = CrashLog.record("main", IllegalStateException("quorrix went sideways"))

        assertTrue(
            "the line must start with the shared grep token and name the thread, got: $record",
            record.startsWith("unhandled on main: "),
        )
    }

    @Test
    fun the_record_carries_the_type_the_message_and_the_stack() {
        val record = CrashLog.record("DefaultDispatcher-worker-1", IllegalStateException("quorrix"))

        assertTrue("the type is missing: $record", record.contains("java.lang.IllegalStateException"))
        assertTrue("the message is missing: $record", record.contains("quorrix"))
        assertTrue("the stack is missing: $record", record.contains("\tat "))
    }

    @Test
    fun a_cause_survives_into_the_record() {
        // A wrapped failure is the common shape, and the cause is the half that says what broke.
        val record = CrashLog.record(
            "main",
            RuntimeException("outer", IllegalArgumentException("quorrix underneath")),
        )

        assertTrue("the cause is missing: $record", record.contains("Caused by"))
        assertTrue("the cause's message is missing: $record", record.contains("quorrix underneath"))
    }

    @Test
    fun the_handler_it_replaced_still_runs() {
        // Android's own default handler shows the crash dialog, reports to Play Console and kills
        // the process. Swallowing it leaves a dead app that never says it died, so this is the
        // assertion that matters more than the text above.
        var seen: Pair<Thread, Throwable>? = null
        val previous = Thread.UncaughtExceptionHandler { thread, throwable ->
            seen = thread to throwable
        }
        val thrown = IllegalStateException("quorrix")

        CrashLog.handler(previous).uncaughtException(Thread.currentThread(), thrown)

        assertEquals(Thread.currentThread() to thrown, seen)
    }

    @Test
    fun no_previous_handler_is_not_a_crash_inside_the_crash_handler() {
        CrashLog.handler(null).uncaughtException(Thread.currentThread(), IllegalStateException("q"))
    }
}
