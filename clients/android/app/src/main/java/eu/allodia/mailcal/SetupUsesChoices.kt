// What the account is used for, chosen on the setup screen: mail, calendar and contacts on the found
// card and the calendar-and-contacts card, colleagues too before a Microsoft or Google sign-in, and
// the uses a JMAP server offers after one (docs/account-autodetect.md rule 8, docs/accounts.md rule
// 10). Which uses are offered and how each starts is the core's; what the toggles make of the
// account is SetupUses, which the JVM suite drives. This file holds the toggles and draws them.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.SetupChoice

/** The toggles for [offer], each starting where the core said, and the servers typed beneath. */
internal class UseToggles(val offer: UseOffer) {
    private fun starts(capability: AccountCapability) = offer.choiceFor(capability)?.on == true

    var mail by mutableStateOf(starts(AccountCapability.MAIL))
    var calendar by mutableStateOf(starts(AccountCapability.CALENDAR))
    var contacts by mutableStateOf(starts(AccountCapability.CONTACTS))
    var colleagues by mutableStateOf(starts(AccountCapability.COLLEAGUES))
    var calendarTyped by mutableStateOf("")
    var contactsTyped by mutableStateOf("")

    /** The server still to be typed, once Connect has asked; marked until it is filled in. */
    var flagged by mutableStateOf(MissingServer.NONE)

    private fun offered(capability: AccountCapability) = offer.choiceFor(capability) != null

    private fun picked(capability: AccountCapability, on: Boolean, typed: String) =
        if (offered(capability)) PickedUse(on, typed) else null

    /** Whether the card offers a choice at all. */
    val choosing: Boolean get() = offer.choices.isNotEmpty()

    /** Whether the mail servers belong on screen: mail is chosen, or nothing is offered. */
    val wantsMail: Boolean get() = !offered(AccountCapability.MAIL) || mail

    /** What the toggles on screen make of the account. */
    fun chosen(): ChosenUses = SetupUses.chosen(
        offer,
        mail.takeIf { offered(AccountCapability.MAIL) },
        picked(AccountCapability.CALENDAR, calendar, calendarTyped),
        picked(AccountCapability.CONTACTS, contacts, contactsTyped),
        colleagues.takeIf { offered(AccountCapability.COLLEAGUES) },
    )

    /**
     * Marks a server still to be typed for a use switched on, and says whether there was one:
     * nothing can be set up until it is filled in.
     */
    fun flagMissing(): Boolean {
        flagged = SetupUses.missing(
            offer,
            picked(AccountCapability.CALENDAR, calendar, calendarTyped),
            picked(AccountCapability.CONTACTS, contacts, contactsTyped),
        )
        return flagged != MissingServer.NONE
    }

    /** How many of mail, calendar and contacts are on: the last one cannot be switched off. */
    private val primaryOn: Int
        get() = listOf(
            AccountCapability.MAIL to mail,
            AccountCapability.CALENDAR to calendar,
            AccountCapability.CONTACTS to contacts,
        ).count { (capability, on) -> offered(capability) && on }

    /** Whether the toggle for a use that is [on] may be flipped. */
    fun canFlip(on: Boolean): Boolean = !on || primaryOn > 1
}

/**
 * The toggles the core offered, in its order, with a server line or field beneath each; those
 * [include] keeps, so a card can draw its mail servers between mail and the rest.
 */
@Composable
internal fun UsesChoices(toggles: UseToggles, include: (AccountCapability) -> Boolean = { true }) {
    val ctx = LocalContext.current
    val offer = toggles.offer
    offer.choices.filter { include(it.capability) }.forEach { choice ->
        when (choice.capability) {
            AccountCapability.MAIL -> {
                SectionHeader("✉", L10n.setup_detect_section_email(ctx))
                UseBox(L10n.setup_detect_mail_enable(ctx), toggles.mail, toggles.canFlip(toggles.mail), "MAIL") {
                    toggles.mail = it
                }
            }
            AccountCapability.CALENDAR -> {
                SectionHeader("📅", L10n.setup_detect_section_calendar(ctx))
                DavUse(
                    choice = choice,
                    found = offer.caldavUrl,
                    enable = L10n.setup_detect_calendar_enable(ctx),
                    add = L10n.setup_detect_calendar_add(ctx),
                    hint = L10n.setup_hint_caldav(ctx),
                    field = L10n.setup_field_caldav(ctx),
                    on = toggles.calendar,
                    enabled = toggles.canFlip(toggles.calendar),
                    onToggle = { toggles.calendar = it },
                    typed = toggles.calendarTyped,
                    onType = {
                        toggles.calendarTyped = it
                        toggles.flagged = MissingServer.NONE
                    },
                    flagged = toggles.flagged == MissingServer.CALENDAR,
                )
            }
            AccountCapability.CONTACTS -> {
                SectionHeader("👤", L10n.setup_detect_section_contacts(ctx))
                DavUse(
                    choice = choice,
                    found = offer.carddavUrl.ifEmpty { offer.caldavUrl },
                    enable = L10n.setup_detect_contacts_enable(ctx),
                    add = L10n.setup_detect_contacts_add(ctx),
                    hint = L10n.setup_hint_carddav(ctx),
                    field = L10n.settings_account_field_carddav(ctx),
                    on = toggles.contacts,
                    enabled = toggles.canFlip(toggles.contacts),
                    onToggle = { on ->
                        toggles.contacts = on
                        // Colleagues come from the directory beside the person's own contacts.
                        if (!on) toggles.colleagues = false
                    },
                    typed = toggles.contactsTyped,
                    onType = {
                        toggles.contactsTyped = it
                        toggles.flagged = MissingServer.NONE
                    },
                    flagged = toggles.flagged == MissingServer.CONTACTS,
                )
            }
            AccountCapability.COLLEAGUES -> Row(modifier = Modifier.padding(start = 24.dp)) {
                UseBox(
                    L10n.setup_detect_colleagues_enable(ctx),
                    toggles.colleagues,
                    toggles.contacts,
                    "COLLEAGUES",
                ) { toggles.colleagues = it }
            }
            // Not offered: this client has no section to choose one (docs/shared-mailboxes.md §3).
            AccountCapability.SHARED_MAILBOXES -> {}
        }
    }
}

@Composable
private fun UseBox(label: String, on: Boolean, enabled: Boolean, tag: String, onChange: (Boolean) -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Checkbox(
            checked = on,
            onCheckedChange = onChange,
            enabled = enabled,
            modifier = Modifier.testTag("setup-use-$tag"),
        )
        Text(label, style = MaterialTheme.typography.bodyMedium)
    }
}

/**
 * A calendar or contacts toggle, and beneath it, following it so a use switched off leaves nothing
 * behind claiming otherwise: the server detection found, by host, or a field for one when it found
 * none. A provider's sign-in covers the use with no server to name.
 */
@Composable
private fun DavUse(
    choice: SetupChoice,
    found: String,
    enable: String,
    add: String,
    hint: String,
    field: String,
    on: Boolean,
    enabled: Boolean,
    onToggle: (Boolean) -> Unit,
    typed: String,
    onType: (String) -> Unit,
    flagged: Boolean,
) {
    val tag = choice.capability.name
    UseBox(if (choice.serverFound) enable else add, on, enabled, tag, onToggle)
    if (!on) return
    when {
        !choice.serverFound -> {
            SetupField(typed, onType, field, hint, KeyboardType.Uri, missing = flagged)
        }
        found.isNotEmpty() -> Text(
            urlHost(found),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(start = 48.dp),
        )
    }
}
