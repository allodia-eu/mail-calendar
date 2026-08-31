// The two signature rules a client owns (docs/signatures.md), and the payload it hands the editor.
// Everything else, storage, sanitising, the `data:`→`cid:` rewrite, the teardown of dangling
// assignments, is shared Rust with its own tests. What is left here is small and worth pinning,
// because both rules are invisible until they are wrong: a reply that seeds the *new message*
// signature looks fine until someone reads the footer, and an explicit choice quietly re-swapped by
// a From change undoes a deliberate act.
//
// Robolectric, for org.json, the functions themselves are plain Kotlin.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.json.JSONObject
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.SignatureBody
import uniffi.mailcal_bindings.SignatureRow
import uniffi.mailcal_bindings.SignatureSlotKind

private val WORK = SignatureBody("sig-work", "<p>Alice, Work</p>", "Alice, Work")
private val PERSONAL = SignatureBody("sig-home", "<p>Alice</p>", "Alice")

/** A library of two, where each account's slot resolution is decided by the test. */
private fun signatures(
    forAccount: (String, SignatureSlotKind) -> SignatureBody? = { _, _ -> null },
) = ComposerSignatures(
    library = listOf(SignatureRow("sig-work", "Work"), SignatureRow("sig-home", "Personal")),
    forAccount = forAccount,
    byId = { id -> listOf(WORK, PERSONAL).firstOrNull { it.id == id } },
)

@RunWith(RobolectricTestRunner::class)
class ComposerSignatureTest {

    /**
     * A reply, a reply-all and a forward share ONE slot (Outlook's grouping): all three continue an
     * existing message, and splitting them produces a setting nobody sets.
     */
    @Test
    fun `only a new message uses the new-message slot`() {
        assertEquals(SignatureSlotKind.NEW_MESSAGE, signatureSlot(RichComposeMode.New))
        assertEquals(SignatureSlotKind.REPLY_FORWARD, signatureSlot(RichComposeMode.Reply))
        assertEquals(SignatureSlotKind.REPLY_FORWARD, signatureSlot(RichComposeMode.ReplyAll))
        assertEquals(SignatureSlotKind.REPLY_FORWARD, signatureSlot(RichComposeMode.Forward))
    }

    /**
     * With no explicit choice the signature FOLLOWS THE SENDER, a work signature going out under a
     * personal address is the mistake the setting exists to prevent, so it is automatic rather than
     * a reminder.
     */
    @Test
    fun `no choice follows the from account, per slot`() {
        val library = signatures { account, slot ->
            when {
                account == "work" && slot == SignatureSlotKind.NEW_MESSAGE -> WORK
                account == "home" -> PERSONAL
                else -> null
            }
        }

        assertEquals(WORK, library.resolve(choice = null, account = "work", mode = RichComposeMode.New))
        assertEquals(PERSONAL, library.resolve(choice = null, account = "home", mode = RichComposeMode.New))
        // The same account, a different mode: its reply slot is unassigned, which is how a user
        // says "no signature on replies from here".
        assertNull(library.resolve(choice = null, account = "work", mode = RichComposeMode.Reply))
        // No account yet (the list has not arrived) resolves to nothing rather than guessing one.
        assertNull(library.resolve(choice = null, account = null, mode = RichComposeMode.New))
    }

    /**
     * An explicit choice survives a From change: the user picked it *for this message*, and silently
     * replacing it would undo a deliberate act. (Outlook re-swaps regardless; it is its most
     * complained-about composer behaviour.)
     */
    @Test
    fun `an explicit choice wins over whatever the account assigns`() {
        val library = signatures { _, _ -> WORK }

        assertEquals(
            PERSONAL,
            library.resolve(SignatureChoice.Named("sig-home"), account = "work", mode = RichComposeMode.New),
        )
        // Including the explicit "None", which must not fall back to the account's.
        assertNull(
            library.resolve(SignatureChoice.NoSignature, account = "work", mode = RichComposeMode.New),
        )
    }

    /** A choice naming a signature that no longer exists resolves to none, not to the account's. */
    @Test
    fun `a stale choice resolves to no signature`() {
        val library = signatures { _, _ -> WORK }

        assertNull(
            library.resolve(SignatureChoice.Named("deleted"), account = "work", mode = RichComposeMode.New),
        )
    }

    /**
     * The seed is the shape the Rust composer's `Block::Signature` round-trips, so what the editor
     * hands back on submit is what the core already understands.
     */
    @Test
    fun `the seed carries both bodies under the keys the core reads`() {
        val json = JSONObject(signatureSeedJson(WORK)!!)

        assertEquals("<p>Alice, Work</p>", json.getString("body_html"))
        assertEquals("Alice, Work", json.getString("body_plain"))
        assertEquals("only the two body keys ride across", 2, json.length())
    }

    /** No signature is a null payload, the editor seam reads that as "remove the region". */
    @Test
    fun `no signature seeds nothing`() {
        assertNull(signatureSeedJson(null))
    }
}
