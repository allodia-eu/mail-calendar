// The manual form's port and connection-security rule, as arithmetic. The Android client's copy
// of the suite every client carries for ManualServerField (docs/account-autodetect.md).
//
// What is load-bearing: the picker fills the port until the user takes it over, and a port they
// typed is never overwritten. A server on a non-standard port is the reason the manual form
// exists, so a picker that resets it would defeat the form. No Compose or device needed.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.ConnectionSecurity

class ManualServerFieldTest {
    // The standard ports, as the core answers them. They are written out here because this suite
    // loads no cdylib; that they are the SAME numbers the core dials is pinned in
    // `mailcal-bindings`'s own suite, which is the only place that can assert it.
    private fun imap() = ManualServerField { security ->
        if (security == ConnectionSecurity.IMPLICIT_TLS) 993 else 143
    }

    private fun smtp() = ManualServerField { security ->
        if (security == ConnectionSecurity.IMPLICIT_TLS) 465 else 587
    }

    @Test
    fun `a fresh field offers the standard secure port`() {
        assertEquals("993", imap().port)
        assertEquals("465", smtp().port)
        assertEquals(ConnectionSecurity.IMPLICIT_TLS, imap().security)
    }

    @Test
    fun `the port follows the picker while it is still ours`() {
        assertEquals("143", imap().choose(ConnectionSecurity.START_TLS).port)
        assertEquals(
            "993",
            imap().choose(ConnectionSecurity.START_TLS)
                .choose(ConnectionSecurity.IMPLICIT_TLS).port,
        )
        assertEquals("587", smtp().choose(ConnectionSecurity.START_TLS).port)
    }

    @Test
    fun `a port typed by hand is never overwritten by the picker`() {
        val field = imap().typePort("1143")
        assertEquals("1143", field.choose(ConnectionSecurity.START_TLS).port)
        assertEquals("1143", field.choose(ConnectionSecurity.IMPLICIT_TLS).port)
        assertFalse(field.followsSecurity)
    }

    @Test
    fun `clearing the port hands it back to the picker`() {
        val field = imap().typePort("1143").typePort("   ")
        assertTrue(field.followsSecurity)
        assertEquals("993", field.port)
        assertEquals("143", field.choose(ConnectionSecurity.START_TLS).port)
    }

    @Test
    fun `the dial address carries the host and the port together`() {
        val starttls = imap().choose(ConnectionSecurity.START_TLS)
        assertEquals("imap.example.net:143", starttls.dial("imap.example.net"))

        val bridge = starttls.typePort("1143")
        assertEquals("127.0.0.1:1143", bridge.dial("127.0.0.1"))
        assertEquals("127.0.0.1:1143", bridge.dial("  127.0.0.1  "))
    }

    @Test
    fun `a port already typed into the host field wins`() {
        assertEquals("127.0.0.1:1025", imap().typePort("1143").dial("127.0.0.1:1025"))
    }

    @Test
    fun `an empty host stays empty so the connect gate still refuses it`() {
        assertEquals("", imap().dial("   "))
    }

    @Test
    fun `a detected route brings its own port and stops following the picker`() {
        val field = imap().adoptDetected("imap.example.net:1993", ConnectionSecurity.START_TLS)
        assertEquals("1993", field.port)
        assertFalse(field.followsSecurity)
        assertEquals(ConnectionSecurity.START_TLS, field.security)
    }

    @Test
    fun `a detected route without a port shows the standard one for what was detected`() {
        val field = imap().adoptDetected("imap.example.net", ConnectionSecurity.START_TLS)
        assertEquals("143", field.port)
        assertTrue(field.followsSecurity)
    }

    @Test
    fun `only an all-digit tail is a port`() {
        assertEquals(
            "imap.example.net" to "",
            ManualServerField.splitHost("imap.example.net"),
        )
        assertEquals(
            "imap.example.net" to "993",
            ManualServerField.splitHost("imap.example.net:993"),
        )
        assertEquals("host:" to "", ManualServerField.splitHost("host:"))
        assertEquals("host:abc" to "", ManualServerField.splitHost("host:abc"))
        // Mirrors the core, which splits the same way; a bare IPv6 literal is not a server
        // either side accepts.
        assertEquals(":" to "1", ManualServerField.splitHost("::1"))
    }
}
