// A core refusal says what went wrong in its own words, never the generated record's `v1=...` form,
// which is what the setup form and an account's page put in front of somebody.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.mailcal_bindings.MailcalException

class CoreErrorTextTest {
    @Test
    fun a_refusal_reads_as_its_message() {
        assertEquals("sign-in rejected", coreErrorText(MailcalException.Connect("sign-in rejected")))
        assertEquals("no such account", coreErrorText(MailcalException.Config("no such account")))
        assertEquals("plain", coreErrorText(IllegalStateException("plain")))
    }
}
