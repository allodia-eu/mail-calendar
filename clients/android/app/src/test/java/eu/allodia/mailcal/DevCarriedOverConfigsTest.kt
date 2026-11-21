// What a dev-account launch carries over from the store, and whether a launch counts as a first
// run. A dev launch connects a canned harness account instead of the stored ones, right for mail,
// because this store is not namespaced per dev account and appending it wholesale would drag the
// developer's real accounts into a harness run; but the Allodia entry and the own AI endpoint's key
// are not mail accounts, and dropping them made a sign-in or an endpoint set up in this mode look
// like it had never stuck. Counted as mail accounts, either would also skip first-run setup for
// somebody with no mailbox.
//
// Every failure here is silent: carrying nothing loses the sign-in at every launch, carrying
// everything opens someone's real mailbox against the harness, and a key counted as an account
// leaves an empty inbox with no way to setup. Which entry is reserved is the core's answer
// (`isReservedConfig`), so it is passed in: this suite never loads the cdylib.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class DevCarriedOverConfigsTest {
    private val allodia = "[allodia]\nemail = \"person@allodia.eu\"\n"
    private val endpoint = "[ai_endpoint]\nkey = \"secret\"\n"
    private val imap = "[imap]\naddr = \"imap.test.local:993\"\n"
    private val jmap = "[jmap]\nbase_url = \"https://api.test.local\"\n"

    /** Stands in for the core: the shapes it recognises are pinned by its own tests, not here. */
    private fun isReserved(config: String) = config.startsWith("[allodia]") || config.startsWith("[ai_endpoint]")

    @Test
    fun the_allodia_account_and_the_endpoint_key_come_over() {
        assertEquals(
            listOf(allodia, endpoint),
            devCarriedOverConfigs(listOf(imap, allodia, jmap, endpoint), ::isReserved),
        )
    }

    @Test
    fun a_mail_account_does_not() {
        assertEquals(
            emptyList<String>(),
            devCarriedOverConfigs(listOf(imap, jmap), ::isReserved),
        )
    }

    @Test
    fun an_empty_store_carries_nothing() {
        assertEquals(emptyList<String>(), devCarriedOverConfigs(emptyList(), ::isReserved))
    }

    @Test
    fun a_store_of_reserved_entries_still_needs_a_mail_account() {
        assertTrue(needsMailSetup(emptyList(), ::isReserved))
        assertTrue(needsMailSetup(listOf(allodia, endpoint), ::isReserved))
        assertTrue("an endpoint key alone is not a mail account", needsMailSetup(listOf(endpoint), ::isReserved))
        assertFalse(needsMailSetup(listOf(endpoint, imap), ::isReserved))
    }
}
