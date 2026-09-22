// The status line's caption: which accounts are named, how the folder counts add up, and what a
// paused account is told to expect.
//
// This line is the only thing the client says about a pass the user did not start, so a wrong
// account name here attributes someone else's mail to the wrong mailbox, a wrong count is the
// difference between "nearly done" and "just started", and a pause that names no wait leaves a
// stalled mailbox looking broken. None of it needs a renderer to prove.
package eu.allodia.mailcal

import android.content.Context
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.AccountSyncProgress
import uniffi.mailcal_bindings.SyncProgressSnapshot
import uniffi.mailcal_bindings.ThrottledAccount

private fun ctx(): Context = RuntimeEnvironment.getApplication()

private fun account(id: String, email: String) =
    AccountRow(id = id, email = email, name = "", expanded = false)

private fun syncing(vararg accounts: AccountSyncProgress) =
    SyncProgressSnapshot(
        active = false,
        fetched = 0uL,
        total = null,
        accounts = accounts.toList(),
        throttled = emptyList(),
    )

private fun paused(vararg accounts: ThrottledAccount) =
    SyncProgressSnapshot(
        active = false,
        fetched = 0uL,
        total = null,
        accounts = emptyList(),
        throttled = accounts.toList(),
    )

private fun waiting(id: String, minutes: UInt?) =
    ThrottledAccount(accountId = id, resumesInMinutes = minutes)

private fun pass(id: String, done: UInt, total: UInt) =
    AccountSyncProgress(
        accountId = id,
        foldersDone = done,
        foldersTotal = total,
        warmingBodies = false,
        bodiesDone = 0u,
    )

private fun warming(id: String, bodies: UInt) =
    AccountSyncProgress(
        accountId = id,
        foldersDone = 0u,
        foldersTotal = 0u,
        warmingBodies = true,
        bodiesDone = bodies,
    )

@RunWith(RobolectricTestRunner::class)
class SyncStatusTest {

    // Nothing arriving unasked is the normal state, and it must render nothing at all rather than
    // an empty strip that still takes its padding.
    @Test
    fun a_quiet_client_shows_no_hint() {
        assertNull(syncStatusCaption(ctx(), null, emptyList()))
        assertNull(syncStatusCaption(ctx(), syncing(), listOf(account("a", "me@example.test"))))
    }

    @Test
    fun one_account_is_named_by_its_address() {
        assertEquals(
            "Syncing me@example.test: 3 of 12 folders",
            syncHintCaption(
                ctx(),
                syncing(pass("a", 3u, 12u)),
                listOf(account("a", "me@example.test"), account("b", "other@example.test")),
            ),
        )
    }

    // An account can be removed while its pass is still winding down. The id is not a nice label,
    // but it is honest, and it is a great deal better than crashing on a missing row.
    @Test
    fun an_account_the_client_no_longer_lists_falls_back_to_its_id() {
        assertEquals(
            "Syncing acct-9: 0 of 4 folders",
            syncHintCaption(ctx(), syncing(pass("acct-9", 0u, 4u)), emptyList()),
        )
    }

    // Several accounts share one line, a footer cannot name them all, and they carry no counts,
    // because one account on its folders and another on its bodies have no shared unit to add up.
    @Test
    fun several_accounts_are_counted_without_a_unit() {
        assertEquals(
            "Syncing 2 accounts",
            syncHintCaption(
                ctx(),
                syncing(pass("a", 3u, 12u), warming("b", 900u)),
                listOf(account("a", "me@example.test"), account("b", "other@example.test")),
            ),
        )
    }

    // The body warm: the longer half of a first sync, and the half that used to say nothing at
    // all. It has no total, the warm drains against what is still missing, so the caption says
    // how far it has got and no more.
    @Test
    fun a_body_warm_reports_how_many_messages_are_down_so_far() {
        assertEquals(
            "Syncing me@example.test: 2,022 messages so far",
            syncHintCaption(
                ctx(),
                syncing(warming("a", 2022u)),
                listOf(account("a", "me@example.test")),
            ),
        )
    }

    // Assert the copy is *Dutch*, not which Dutch: the JDK's locale data decides the rest.
    @Test
    @Config(qualifiers = "nl")
    fun both_forms_are_translated() {
        assertEquals(
            "me@example.test synchroniseren: 3 van 12 mappen",
            syncHintCaption(
                ctx(),
                syncing(pass("a", 3u, 12u)),
                listOf(account("a", "me@example.test")),
            ),
        )
        assertEquals(
            "2 accounts synchroniseren",
            syncHintCaption(ctx(), syncing(pass("a", 3u, 12u), pass("b", 1u, 5u)), emptyList()),
        )
        assertEquals(
            "me@example.test synchroniseren: 2.022 berichten tot nu toe",
            syncHintCaption(
                ctx(),
                syncing(warming("a", 2022u)),
                listOf(account("a", "me@example.test")),
            ),
        )
    }

    // A server answered promptly and asked to be left alone. A phone has no hover, so the line
    // says the whole sentence rather than a label with a tooltip behind it.
    @Test
    fun a_paused_account_says_when_syncing_continues() {
        assertEquals(
            "The mail server for me@example.test asked us to slow down. " +
                "Syncing continues in about 5 minutes.",
            syncStatusCaption(
                ctx(),
                paused(waiting("a", 5u)),
                listOf(account("a", "me@example.test")),
            ),
        )
    }

    // Two refusals in three name no instant. "Shortly" is the honest answer; a figure here would
    // be one we made up.
    @Test
    fun a_pause_the_server_did_not_time_says_so_rather_than_inventing_a_figure() {
        assertEquals(
            "The mail server for me@example.test asked us to slow down. " +
                "Syncing continues shortly.",
            syncStatusCaption(
                ctx(),
                paused(waiting("a", null)),
                listOf(account("a", "me@example.test")),
            ),
        )
    }

    // The singular has to be picked by the count, not spelled into the sentence: a plural
    // accessor that ignored it reads "in about 1 minutes".
    @Test
    fun a_one_minute_wait_reads_as_one_minute() {
        assertEquals(
            "The mail server for me@example.test asked us to slow down. " +
                "Syncing continues in about 1 minute.",
            syncStatusCaption(
                ctx(),
                paused(waiting("a", 1u)),
                listOf(account("a", "me@example.test")),
            ),
        )
    }

    // A status line cannot name them all, and their waits have no shared end to state.
    @Test
    fun several_paused_accounts_give_way_to_a_count() {
        assertEquals(
            "The mail servers for 2 accounts asked us to slow down. Syncing continues shortly.",
            syncStatusCaption(ctx(), paused(waiting("a", 5u), waiting("b", null)), emptyList()),
        )
    }
}
