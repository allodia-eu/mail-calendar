// The settings hub-and-spoke contract: the Android Settings screen shows the same categories, in
// the same order, under the same names as the macOS sidebar and the Windows source-list
// (docs/settings.md). That sameness is what lets a support answer name one path, "Settings →
// Reading", and have it be true on every platform, so it is pinned here rather than left to a
// review to notice. Driven through the real composable with Compose's test rule; nothing loads
// the cdylib (the generated binding's records/enums are plain Kotlin).
package eu.allodia.mailcal

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.mailcal_bindings.Appearance
import uniffi.mailcal_bindings.CalendarLayout
import uniffi.mailcal_bindings.DisplaySettings
import uniffi.mailcal_bindings.QuoteSettings
import uniffi.mailcal_bindings.AboutInfo
import uniffi.mailcal_bindings.AllodiaAccount
import uniffi.mailcal_bindings.AccountSignatureRow
import uniffi.mailcal_bindings.Attribution
import uniffi.mailcal_bindings.QuoteStyleKind
import uniffi.mailcal_bindings.SignatureRow
import uniffi.mailcal_bindings.SignatureSlotKind
import uniffi.mailcal_bindings.SignaturesSnapshot
import uniffi.mailcal_bindings.SwipeActionKind
import uniffi.mailcal_bindings.SwipeSettings
import uniffi.mailcal_bindings.TimeFormat
import uniffi.mailcal_bindings.ViewMode
import uniffi.mailcal_bindings.WeekStart

/**
 * The About payload the screen renders. A record, not a core call: this suite never loads the
 * cdylib, and `aboutInfo` is a call into it, the shape is what the screen has to draw, and the
 * content is the core's own test (`crates/mailcal-bindings/src/about.rs`).
 */
private val ABOUT = AboutInfo(
    version = "9.9.9",
    supportUrl = "https://support.allodia.eu",
    attributions = listOf(Attribution("Rust", "MIT OR Apache-2.0")),
)

@RunWith(RobolectricTestRunner::class)
class SettingsHubTest {
    @get:Rule val compose = createComposeRule()

    /** How often the screen asked to leave settings entirely (Done, or confirming a reset). */
    private var closed = 0
    private var resets = 0
    private var diagnosticsOpened = 0

    /** The signature snapshot the screen renders; overridden by the tests that need one. */
    private var signatures: SignaturesSnapshot? = null

    private fun show(
        initialCategory: SettingsCategory? = null,
        allodia: AllodiaSettings = AllodiaSettings(),
        allodiaSync: AllodiaSyncState = AllodiaSyncState(),
    ) {
        compose.setContent {
            SettingsScreen(
                about = ABOUT,
                timeZone = null,
                onSetTimeZone = {},
                display = DisplaySettings(
                    WeekStart.MONDAY,
                    TimeFormat.TWENTY_FOUR_HOUR,
                    Appearance.SYSTEM,
                    12u,
                    CalendarLayout.WEEK,
                ),
                onSetTimeFormat = {},
                onSetAppearance = {},
                onSetWeekStart = {},
                onSetVisibleHours = {},
            calendars = emptyList(),
            onSetDefaultCalendar = { _, _ -> },
                mode = ViewMode.THREADED,
                onSetMode = {},
                quoteSettings = QuoteSettings(QuoteStyleKind.INDENTED, perMessage = false),
                onSetQuoteStyle = {},
                onSetQuoteStylePerMessage = {},
                accounts = emptyList(),
                defaultSendAccount = null,
                onSetDefaultSendAccount = {},
                swipe = SwipeSettings(SwipeActionKind.ARCHIVE, SwipeActionKind.DELETE),
                onSetSwipeLeft = {},
                onSetSwipeRight = {},
                signatures = signatures,
                signatureHtml = { null },
                onCreateSignature = { _, _, _ -> },
                onUpdateSignature = { _, _, _, _ -> },
                onDeleteSignature = {},
                onSetAccountSignature = { _, _, _ -> },
                analyticsEnabled = false,
                onSetAnalytics = {},
                analyticsPayloadPreview = { "{}" },
                // Defaults to a build from source: no Allodia route, so the category is absent.
                allodia = allodia,
                onAllodiaSignIn = {},
                onAllodiaCreate = {},
                onAllodiaManage = {},
                onAllodiaSignOut = {},
                allodiaSync = allodiaSync,
                onAllodiaSetUp = {},
                onAllodiaKeepLocal = {},
                accountsSyncMode = emptyMap(),
                onSetAccountSyncMode = { _, _ -> },
                settings = null,
                onSetSyncDepth = { _, _ -> },
                onSetMessageSize = { _, _ -> },
                onSetStrategy = { _, _ -> },
                onSetPollInterval = { _, _ -> },
                onSetPushFolder = { _, _, _ -> },
                onOpenDiagnostics = { diagnosticsOpened++ },
                onReset = { resets++ },
                onBack = { closed++ },
                initialCategory = initialCategory,
            )
        }
    }

    private fun ctx() = RuntimeEnvironment.getApplication()

    /**
     * The order is the contract, not a style choice: it must match the macOS sidebar and the
     * Windows source-list (with NOTIFICATIONS slotted in, mobile-only) so one support answer fits
     * every platform.
     */
    @Test
    fun the_taxonomy_keeps_the_shared_desktop_order() {
        assertEquals(
            listOf(
                SettingsCategory.ALLODIA,
                SettingsCategory.GENERAL,
                SettingsCategory.CALENDAR,
                SettingsCategory.READING,
                SettingsCategory.COMPOSING,
                SettingsCategory.SIGNATURES,
                SettingsCategory.NOTIFICATIONS,
                SettingsCategory.PRIVACY,
                SettingsCategory.ACCOUNTS,
                SettingsCategory.ADVANCED,
                SettingsCategory.DIAGNOSTICS,
                SettingsCategory.ABOUT,
            ),
            SettingsCategory.entries.toList(),
        )
    }

    /**
     * About is the one category whose content is not a setting: it is what a support answer needs
     * quoted back at it. The version has to be the core's, not a string the client keeps, and each
     * attribution has to name the licence it is used under, a notice nobody can read is none.
     */
    @Test
    fun about_states_the_core_s_version_the_support_forum_and_every_attribution() {
        show()
        compose.onNodeWithText(L10n.settings_category_about(ctx())).performScrollTo().performClick()

        compose.onNodeWithText(L10n.about_version(ctx(), ABOUT.version)).assertIsDisplayed()
        compose.onNodeWithText(ABOUT.supportUrl).assertIsDisplayed()
        for (item in ABOUT.attributions) {
            compose.onNodeWithText(item.name).performScrollTo().assertIsDisplayed()
            compose.onNodeWithText(item.license).performScrollTo().assertIsDisplayed()
        }
        assertEquals("About opens in place, like every other category", 0, diagnosticsOpened)
    }

    @Test
    fun the_hub_lists_every_category_with_its_summary() {
        show()
        for (category in SettingsCategory.shown(allodiaAvailable = false)) {
            compose.onNodeWithText(category.title(ctx())).performScrollTo().assertIsDisplayed()
            compose.onNodeWithText(category.summary(ctx())).performScrollTo().assertIsDisplayed()
        }
    }

    /**
     * A build carrying no Allodia registration loses the whole category, not just its contents.
     *
     * The failure this pins is a quiet one: a hub row that opens an empty screen looks like a bug
     * in the screen rather than in the gating, and it is exactly what every contributor building
     * from source would see first.
     */
    @Test
    fun a_build_without_the_registration_has_no_allodia_category_at_all() {
        show()
        compose.onAllNodesWithText(L10n.settings_allodia_heading(ctx())).assertCountEquals(0)
        compose.onNodeWithText(L10n.settings_category_general(ctx())).assertIsDisplayed()
        assertEquals(
            "General is first when the account category is absent",
            SettingsCategory.GENERAL,
            SettingsCategory.shown(allodiaAvailable = false).first(),
        )
    }

    @Test
    fun a_build_with_the_registration_puts_the_account_first() {
        show(allodia = AllodiaSettings(available = true))
        compose.onNodeWithText(L10n.settings_allodia_heading(ctx())).assertIsDisplayed()
        assertEquals(
            SettingsCategory.ALLODIA,
            SettingsCategory.shown(allodiaAvailable = true).first(),
        )
    }

    /**
     * Signed out, both routes are offered. Someone who has no account and someone returning to one
     * need different pages, and a single "Sign in" sends the first of them through a form they did
     * not want.
     */
    @Test
    fun signed_out_offers_both_signing_in_and_creating_an_account() {
        show(
            initialCategory = SettingsCategory.ALLODIA,
            allodia = AllodiaSettings(available = true),
        )
        compose.onNodeWithText(L10n.settings_allodia_sign_in(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.settings_allodia_create(ctx())).assertIsDisplayed()
    }

    /**
     * Signed in, the account page is reachable and deletion is named on the screen.
     *
     * "Manage account" is not the word anybody looks for when they want out, and an app that lets
     * someone create an account has to offer deletion somewhere findable.
     */
    @Test
    fun signed_in_names_the_address_and_offers_managing_deleting_and_leaving() {
        show(
            initialCategory = SettingsCategory.ALLODIA,
            allodia = AllodiaSettings(
                available = true,
                account = AllodiaAccount("someone@example.com", null),
            ),
        )
        compose
            .onNodeWithText(L10n.settings_allodia_signed_in(ctx(), "someone@example.com"))
            .assertIsDisplayed()
        compose.onNodeWithText(L10n.settings_allodia_manage(ctx())).performScrollTo()
            .assertIsDisplayed()
        compose.onNodeWithText(L10n.settings_allodia_delete(ctx())).performScrollTo()
            .assertIsDisplayed()
        compose.onNodeWithText(L10n.settings_allodia_sign_out(ctx())).performScrollTo()
            .assertIsDisplayed()
    }

    @Test
    fun a_category_opens_its_own_screen_and_the_back_arrow_returns_to_the_hub() {
        show()
        compose.onNodeWithText(L10n.settings_category_calendar(ctx())).performClick()

        // The category's settings are showing, and the hub's other rows are gone, this is a
        // screen of its own, not a scroll anchor in one long list.
        compose.onNodeWithText(L10n.settings_week_start_heading(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.settings_category_reading(ctx())).assertDoesNotExist()

        compose.onNodeWithContentDescription(L10n.a11y_back(ctx())).performClick()
        compose.onNodeWithText(L10n.settings_category_reading(ctx())).assertIsDisplayed()
        assertEquals("returning to the hub must not leave settings", 0, closed)
    }

    /**
     * Diagnostics is the one category without an inline detail: its log viewer is a full screen
     * swapped in at the activity level, so the hub row calls onOpenDiagnostics rather than opening
     * a detail pane (and so the hub stays put).
     */
    @Test
    fun the_diagnostics_row_opens_the_standalone_screen() {
        show()
        compose.onNodeWithText(L10n.settings_category_diagnostics(ctx())).performScrollTo().performClick()
        assertEquals(1, diagnosticsOpened)
        // No inline detail opened, a detail screen shows the back arrow, the hub does not, so the
        // row handed off to the standalone screen rather than swapping in a CategoryDetail.
        compose.onNodeWithContentDescription(L10n.a11y_back(ctx())).assertDoesNotExist()
        assertEquals("opening diagnostics must not leave settings", 0, closed)
    }

    /**
     * The library comes FIRST, above the per-account defaults: an account picker with nothing to
     * pick says nothing, so a first-time user has to write a signature before the defaults mean
     * anything (docs/signatures.md). With an empty library the screen says so in words rather than
     * showing an empty box.
     */
    @Test
    fun the_signatures_category_leads_with_the_library() {
        show()
        compose.onNodeWithText(L10n.settings_category_signatures(ctx())).performScrollTo().performClick()

        compose.onNodeWithText(L10n.settings_signatures_library_heading(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.settings_signatures_empty(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.settings_signatures_add(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.settings_signatures_defaults_heading(ctx()))
            .performScrollTo()
            .assertIsDisplayed()
    }

    /**
     * Two slots per account, each independently set, each offering "None", and there is deliberately
     * NO separate "signatures on" switch: None in both already says "this account sends no
     * signature", and a second control that could disagree with the pickers is a bug waiting to
     * happen.
     */
    @Test
    fun each_account_gets_both_slots_and_none_is_a_real_choice() {
        signatures = SignaturesSnapshot(
            signatures = listOf(SignatureRow("sig-work", "Work")),
            accounts = listOf(
                AccountSignatureRow("acct", "alice@test.local", newMessage = "sig-work", replyForward = null),
            ),
        )
        show()
        compose.onNodeWithText(L10n.settings_category_signatures(ctx())).performScrollTo().performClick()

        compose.onNodeWithText("alice@test.local").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText(L10n.settings_signatures_new_message_label(ctx()))
            .performScrollTo()
            .assertIsDisplayed()
        compose.onNodeWithText(L10n.settings_signatures_reply_forward_label(ctx()))
            .performScrollTo()
            .assertIsDisplayed()
        // "Work" twice: the library row it was written in, and the slot it is assigned to. The
        // unassigned slot reads "None" rather than sitting blank.
        compose.onAllNodesWithText("Work").assertCountEquals(2)
        compose.onNodeWithText(L10n.settings_signatures_none(ctx())).performScrollTo().assertIsDisplayed()
    }

    /** Clearing a slot dispatches a null assignment, that is how a user turns a signature off. */
    @Test
    fun choosing_none_clears_the_slot() {
        var cleared: Pair<SignatureSlotKind, String?>? = null
        signatures = SignaturesSnapshot(
            signatures = listOf(SignatureRow("sig-work", "Work")),
            accounts = listOf(
                AccountSignatureRow("acct", "alice@test.local", newMessage = "sig-work", replyForward = "sig-work"),
            ),
        )
        compose.setContent {
            AccountSignatureDefaultsCard(
                accounts = signatures!!.accounts,
                signatures = signatures!!.signatures,
                onSet = { _, slot, signature -> cleared = slot to signature },
            )
        }

        // The first slot's control, opened and set to None.
        compose.onAllNodesWithText("Work")[0].performClick()
        compose.onNodeWithText(L10n.settings_signatures_none(ctx())).performClick()

        assertEquals(SignatureSlotKind.NEW_MESSAGE to null, cleared)
    }

    @Test
    fun done_leaves_settings_from_the_hub() {
        show()
        compose.onNodeWithText(L10n.action_done(ctx())).performClick()
        assertEquals(1, closed)
    }

    /** The destructive reset stays behind its confirmation dialog after the hub rework. */
    @Test
    fun resetting_still_takes_a_confirmation() {
        show()
        compose.onNodeWithText(L10n.settings_category_advanced(ctx())).performScrollTo().performClick()
        // The button, not the group heading that shares its label.
        compose.onNode(hasText(L10n.action_reset_database(ctx())) and hasClickAction()).performClick()
        assertEquals("no reset before the dialog is confirmed", 0, resets)

        compose.onNodeWithText(L10n.reset_confirm(ctx())).performClick()
        assertEquals(1, resets)
        assertEquals("a confirmed reset leaves settings", 1, closed)
    }

    @Test
    @Config(qualifiers = "nl")
    fun the_hub_is_translated() {
        show()
        compose.onNodeWithText("Instellingen").assertIsDisplayed()
        compose.onNodeWithText("Agenda").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Meldingen").performScrollTo().assertIsDisplayed()
    }

    /** The notifications settings copy used to be hardcoded English (docs/background-sync.md). */
    @Test
    @Config(qualifiers = "nl")
    fun the_notifications_settings_are_no_longer_english_only() {
        show()
        compose.onNodeWithText("Meldingen").performScrollTo().performClick()
        compose.onNodeWithText("Meldingen bij nieuwe e-mail").assertIsDisplayed()
        compose.onNodeWithText("New-mail notifications").assertDoesNotExist()
    }

    @Test
    fun a_shortcut_opens_straight_on_the_category_it_named() {
        // What the calendar's "Calendar settings" menu entry does: the settings governing that
        // screen are otherwise three taps into a hub the user has to leave the calendar to reach.
        show(initialCategory = SettingsCategory.CALENDAR)
        compose.onNodeWithText(L10n.settings_week_start_heading(ctx())).assertIsDisplayed()
    }

    @Test
    fun backing_out_of_a_shortcut_lands_on_the_hub_not_out_of_settings() {
        // Arriving deep must not make the first back press leave Settings altogether, a phone user
        // expects to surface one level at a time, and the hub is that level.
        show(initialCategory = SettingsCategory.CALENDAR)
        compose.onNodeWithContentDescription(L10n.a11y_back(ctx())).performClick()
        compose.onNodeWithText(L10n.settings_category_reading(ctx())).assertIsDisplayed()
        assertEquals("backing out of a shortcut must not leave settings", 0, closed)
    }

    @Test
    fun opening_the_hub_normally_still_starts_at_the_hub() {
        show()
        compose.onNodeWithText(L10n.settings_title(ctx())).assertIsDisplayed()
    }

}
