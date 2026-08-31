// What a dev-account launch carries over from the store. It connects a canned harness account
// instead of the stored ones, right for mail, because this store is not namespaced per dev account
// and appending it wholesale would drag the developer's real accounts into a harness run, but the
// Allodia entry is not a mail account, and dropping it made a sign-in made in this mode look like it
// had never stuck.
//
// Both failure modes are silent: carrying nothing loses the sign-in at every launch, and carrying
// everything opens someone's real mailbox against the harness. Which entry is the Allodia one is the
// core's answer, so it is passed in, this suite never loads the cdylib.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Test

class DevCarriedOverConfigsTest {
    private val allodia = "[allodia]\nemail = \"person@allodia.eu\"\n"
    private val imap = "[imap]\naddr = \"imap.example.com:993\"\n"
    private val jmap = "[jmap]\nbase_url = \"https://api.example.com\"\n"

    /** Stands in for the core: the shape it recognises is pinned by its own tests, not here. */
    private fun isAllodia(config: String) = config.startsWith("[allodia]")

    @Test
    fun the_allodia_account_comes_over() {
        assertEquals(
            listOf(allodia),
            devCarriedOverConfigs(listOf(imap, allodia, jmap), ::isAllodia),
        )
    }

    @Test
    fun a_mail_account_does_not() {
        assertEquals(
            emptyList<String>(),
            devCarriedOverConfigs(listOf(imap, jmap), ::isAllodia),
        )
    }

    @Test
    fun an_empty_store_carries_nothing() {
        assertEquals(emptyList<String>(), devCarriedOverConfigs(emptyList(), ::isAllodia))
    }
}
