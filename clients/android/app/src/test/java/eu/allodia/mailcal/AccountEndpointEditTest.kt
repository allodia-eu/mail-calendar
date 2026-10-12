// Editing an account's servers in Settings (docs/accounts.md rule 14): what the fields show for a
// stored address, and what a save sends back.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AccountEndpoints
import uniffi.mailcal_bindings.ConnectionSecurity

class AccountEndpointEditTest {
    private val imap = { security: ConnectionSecurity ->
        if (security == ConnectionSecurity.IMPLICIT_TLS) 993 else 143
    }
    private val smtp = { security: ConnectionSecurity ->
        if (security == ConnectionSecurity.IMPLICIT_TLS) 465 else 587
    }

    private fun mailbox() = AccountEndpoints(
        imapHost = "imap.example.org",
        imapSecurity = ConnectionSecurity.IMPLICIT_TLS,
        smtpHost = "smtp.example.org",
        smtpSecurity = ConnectionSecurity.START_TLS,
        caldavUrl = null,
        carddavUrl = null,
        username = "alice",
        password = null,
    )

    private fun fields(password: String = "") = EndpointFields(
        "imap.example.org", null, "smtp.example.org", null, null, null, "alice", password,
    )

    @Test
    fun an_empty_password_keeps_the_stored_one_and_a_typed_one_replaces_it() {
        assertEquals(mailbox(), AccountEndpointEdit.edited(mailbox(), fields()))

        val changed = fields("new secret").copy(imapSecurity = ConnectionSecurity.START_TLS)
        val endpoints = AccountEndpointEdit.edited(mailbox(), changed)
        assertEquals("new secret", endpoints.password)
        assertEquals(ConnectionSecurity.START_TLS, endpoints.imapSecurity)
    }

    @Test
    fun a_server_the_account_does_not_have_is_never_added_by_the_form() {
        val typed = EndpointFields(null, null, null, null, "https://dav.example.org", null, "alice", "")
        val endpoints = AccountEndpointEdit.edited(mailbox(), typed)
        assertNull(endpoints.caldavUrl)
        // And a server with no field keeps what it had.
        assertEquals("imap.example.org", endpoints.imapHost)
    }

    @Test
    fun a_default_port_is_stored_bare() {
        val tls = ConnectionSecurity.IMPLICIT_TLS
        val starttls = ConnectionSecurity.START_TLS
        assertEquals("smtp.example.org", AccountEndpointEdit.joined("smtp.example.org", "587", starttls, smtp))
        assertEquals("imap.example.org", AccountEndpointEdit.joined("imap.example.org", "993", tls, imap))
        assertEquals("imap.example.org", AccountEndpointEdit.joined("imap.example.org", "", tls, imap))
        assertEquals("imap.example.org:1143", AccountEndpointEdit.joined(" imap.example.org ", "1143", tls, imap))
        // A port that is the standard one for the other security is not this one's.
        assertEquals("imap.example.org:143", AccountEndpointEdit.joined("imap.example.org", "143", tls, imap))
        assertEquals("imap.example.org", AccountEndpointEdit.joined("imap.example.org", "143", starttls, imap))
    }

    @Test
    fun a_port_written_into_the_host_wins() {
        // As the setup form dials it: two ports would be a contradiction, and the one in the host
        // field is the one the person can see beside the name.
        assertEquals(
            "mail.example:2993",
            AccountEndpointEdit.joined("mail.example:2993", "1234", ConnectionSecurity.IMPLICIT_TLS, imap),
        )
    }

    @Test
    fun a_server_left_empty_is_not_sent() {
        assertTrue(AccountEndpointEdit.complete(mailbox(), fields()))
        assertFalse(AccountEndpointEdit.complete(mailbox(), fields().copy(imapHost = " ")))
        assertFalse(AccountEndpointEdit.complete(mailbox(), fields().copy(username = "")))
        // A server the account does not have is not asked for.
        assertTrue(AccountEndpointEdit.complete(mailbox(), fields().copy(caldavUrl = "")))
    }
}
