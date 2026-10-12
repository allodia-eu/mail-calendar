// The setup screens' choice of what an account is used for (docs/account-autodetect.md rule 8,
// docs/accounts.md rules 2 and 10), as a person meets it: the found card's toggles, the card for a
// domain with only a calendar and address book, the manual form's calendar-and-contacts type, a
// provider sign-in asking for what was chosen alone, and the link step that ends setup.
package eu.allodia.mailcal

import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountCapability.CALENDAR
import uniffi.mailcal_bindings.AccountCapability.COLLEAGUES
import uniffi.mailcal_bindings.AccountCapability.CONTACTS
import uniffi.mailcal_bindings.AccountCapability.MAIL
import uniffi.mailcal_bindings.AccountSetup
import uniffi.mailcal_bindings.ConnectionSecurity
import uniffi.mailcal_bindings.DetectedServerRow
import uniffi.mailcal_bindings.DetectedSetup
import uniffi.mailcal_bindings.LinkSlot
import uniffi.mailcal_bindings.MissReason
import uniffi.mailcal_bindings.SetupRecommendation

@RunWith(RobolectricTestRunner::class)
class SetupCardsTest {
    @get:Rule val compose = createComposeRule()

    private val connected = mutableListOf<AccountSetup>()
    private val signIns = mutableListOf<List<AccountCapability>?>()

    private fun imap(caldav: String?) = SetupRecommendation.Imap(
        email = "alice@example.org",
        imapHost = "imap.example.org",
        smtpHost = "smtp.example.org",
        imapSecurity = ConnectionSecurity.IMPLICIT_TLS,
        smtpSecurity = ConnectionSecurity.IMPLICIT_TLS,
        incoming = DetectedServerRow("IMAP", "imap.example.org", 993u, "SSL/TLS", "alice@example.org"),
        outgoing = DetectedServerRow("SMTP", "smtp.example.org", 465u, "SSL/TLS", "alice@example.org"),
        caldavUrl = caldav,
        oauthIssuer = null,
        isTrusted = true,
        source = "autoconfig",
    )

    private fun flow(setup: DetectedSetup) {
        compose.setContent {
            AccountSetupFlow(
                externalError = null,
                onCancel = null,
                signingIn = false,
                connecting = false,
                detect = { setup },
                onSignInMicrosoft = { _, uses -> signIns += uses },
                onSignInGoogle = { _, uses -> signIns += uses },
                onConnect = {
                    connected += it
                    null
                },
                onConnectJmap = { null },
            )
        }
        compose.onNodeWithText("Email").performTextInput("alice@example.org")
        compose.onNodeWithText("Continue").performClick()
        compose.waitForIdle()
    }

    private fun tap(tag: String) {
        compose.onNodeWithTag(tag).performScrollTo().performClick()
        compose.waitForIdle()
    }

    private fun password() {
        compose.onNodeWithText("Password").performScrollTo().performTextInput("secret")
    }

    @Test
    fun an_account_used_for_its_calendar_alone_keeps_no_mail_servers() {
        flow(standardsSetup(imap("https://dav.example.org/")))

        compose.onNodeWithText("IMAP · imap.example.org:993 · SSL/TLS").assertExists()
        tap("setup-use-CONTACTS")
        tap("setup-use-MAIL")
        // The mail servers follow the mail toggle.
        compose.onNodeWithText("IMAP · imap.example.org:993 · SSL/TLS").assertDoesNotExist()
        password()
        compose.onNodeWithText("Connect").performScrollTo().performClick()

        val setup = connected.single()
        assertEquals(listOf(CALENDAR), setup.uses)
        assertEquals("https://dav.example.org/", setup.caldavBaseUrl)
        assertNull(setup.carddavBaseUrl)
    }

    @Test
    fun a_use_switched_on_without_a_server_waits_for_one() {
        flow(standardsSetup(imap(null)))
        tap("setup-use-CALENDAR")
        password()
        compose.onNodeWithText("Connect").performScrollTo().performClick()
        assertTrue(connected.isEmpty())

        compose.onNodeWithText("Calendar server (CalDAV)").performScrollTo().performTextInput("cal.example.org")
        compose.onNodeWithText("Connect").performScrollTo().performClick()
        assertEquals("cal.example.org", connected.single().caldavBaseUrl)
        assertEquals(listOf(MAIL, CALENDAR), connected.single().uses)
    }

    @Test
    fun a_domain_with_only_a_calendar_and_address_book_is_set_up_for_those() {
        flow(
            standardsSetup(
                SetupRecommendation.Manual(MissReason.NOTHING_FOUND),
                caldav = "https://dav.example.org/",
                calendarAndContacts = true,
            ),
        )
        password()
        compose.onNodeWithText("Connect").performScrollTo().performClick()

        val setup = connected.single()
        // The typed address is the login, and no mail server is stored.
        assertEquals("alice@example.org", setup.username)
        assertEquals("", setup.imapHost)
        assertEquals(listOf(CALENDAR, CONTACTS), setup.uses)
    }

    @Test
    fun a_provider_is_asked_for_what_was_chosen_alone() {
        val choices = listOf(MAIL, CALENDAR, CONTACTS, COLLEAGUES).map { choice(it, true) }
        flow(DetectedSetup(SetupRecommendation.Microsoft("alice@example.org"), false, null, null, choices))
        tap("setup-use-CALENDAR")
        // Colleagues come from the directory beside contacts, so they go with them.
        tap("setup-use-CONTACTS")
        compose.onNodeWithText("Sign in with Microsoft").performScrollTo().performClick()

        assertEquals(listOf(listOf(MAIL)), signIns)
    }

    @Test
    fun the_manual_form_sets_up_a_calendar_and_contacts_server() {
        compose.setContent {
            AccountSetupScreen(
                onConnect = {
                    connected += it
                    null
                },
                onConnectJmap = { null },
                initialKind = AccountKind.DAV,
                prefillEmail = "alice@example.org",
            )
        }
        password()
        compose.onNodeWithText("Connect").performScrollTo().performClick()
        // No server typed is no account.
        assertTrue(connected.isEmpty())

        compose.onNodeWithText("Address book server, CardDAV URL (optional)").performScrollTo()
            .performTextInput("book.example.org")
        compose.onNodeWithText("Connect").performScrollTo().performClick()
        val setup = connected.single()
        assertEquals(listOf(CONTACTS), setup.uses)
        assertEquals("book.example.org", setup.carddavBaseUrl)
        assertEquals("", setup.imapHost)
    }

    @Test
    fun the_link_step_links_what_is_picked_on_continue_alone() {
        val step = SetupLinkStep.of(
            uniffi.mailcal_bindings.AccountsSnapshot(
                listOf(
                    mailboxOnly(
                        candidates = uniffi.mailcal_bindings.LinkCandidates(
                            listOf(linked("alice@dav")),
                            emptyList(),
                            emptyList(),
                            listOf("alice@dav"),
                        ),
                    ),
                ),
            ),
            mailboxOnly().id,
            { true },
        )!!
        val done = mutableListOf<Boolean>()
        var another = 0
        compose.setContent {
            SetupLinkStepScreen(
                step = step,
                onPick = { _, _ -> },
                onAddAnother = { another++ },
                onBeside = {},
                onDone = { done += it },
            )
        }
        // The suggestion starts picked.
        compose.onNodeWithText("alice@dav").assertExists()
        assertEquals(listOf(LinkSlot.CALENDAR to "alice@dav"), step.links())
        tap("setup-links-add")
        tap("setup-links-continue")
        // One answer: a second press would link and ask the names twice.
        tap("setup-links-continue")
        tap("setup-links-skip")
        assertEquals(1, another)
        assertEquals(listOf(true), done)
    }
}
