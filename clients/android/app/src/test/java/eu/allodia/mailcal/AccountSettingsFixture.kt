// Snapshot entries for the Settings → Accounts suites: an account as the core lists it, built
// without the cdylib, which no JVM test loads.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountDistinction
import uniffi.mailcal_bindings.AccountEndpoints
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.AccountLinkPicker
import uniffi.mailcal_bindings.AccountLinksView
import uniffi.mailcal_bindings.AccountMailSettings
import uniffi.mailcal_bindings.AccountPage
import uniffi.mailcal_bindings.AccountSenderName
import uniffi.mailcal_bindings.AccountServers
import uniffi.mailcal_bindings.AccountUse
import uniffi.mailcal_bindings.AccountUseBlock
import uniffi.mailcal_bindings.AccountUseStatus
import uniffi.mailcal_bindings.AccountUseSwitch
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.ConnectionSecurity
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.LinkCandidates
import uniffi.mailcal_bindings.LinkSlot
import uniffi.mailcal_bindings.LinkedAccount
import uniffi.mailcal_bindings.SignInMethod
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

internal fun linked(id: String, distinction: AccountDistinction? = null) =
    LinkedAccount(id, id, distinction)

internal fun accountEntry(
    kind: CoreAccountKind,
    vararg uses: Pair<AccountCapability, CapabilityState>,
    id: String = "alice@example.org@imap.example.org",
    address: String = "alice@example.org",
    links: AccountLinksView = AccountLinksView(null, null, null),
    linkedFrom: List<LinkedAccount> = emptyList(),
    candidates: LinkCandidates = LinkCandidates(emptyList(), emptyList(), emptyList(), emptyList()),
    endpoints: AccountEndpoints? = null,
    signIn: SignInMethod = SignInMethod.PASSWORD,
    distinction: AccountDistinction? = null,
    nextcloud: Boolean = false,
) = AccountEntry(
    id = id,
    address = address,
    kind = kind,
    signIn = signIn,
    uses = uses.map { (capability, state) -> AccountUse(capability, state) },
    links = links,
    linkedFrom = linkedFrom,
    linkCandidates = candidates,
    endpoints = endpoints,
    distinction = distinction,
    nextcloud = nextcloud,
)

internal fun mailboxOnly(
    links: AccountLinksView = AccountLinksView(null, null, null),
    linkedFrom: List<LinkedAccount> = emptyList(),
    candidates: LinkCandidates = LinkCandidates(emptyList(), emptyList(), emptyList(), emptyList()),
    endpoints: AccountEndpoints? = null,
) = accountEntry(
    CoreAccountKind.IMAP,
    AccountCapability.MAIL to CapabilityState.ON,
    AccountCapability.CALENDAR to CapabilityState.OFF,
    AccountCapability.CONTACTS to CapabilityState.OFF,
    links = links,
    linkedFrom = linkedFrom,
    candidates = candidates,
    endpoints = endpoints,
)

internal fun imapEndpoints() = AccountEndpoints(
    imapHost = "imap.example.org",
    imapSecurity = ConnectionSecurity.IMPLICIT_TLS,
    smtpHost = "smtp.example.org:2525",
    smtpSecurity = ConnectionSecurity.START_TLS,
    caldavUrl = null,
    carddavUrl = null,
    username = "alice@example.org",
    password = null,
)

/** A control nothing holds. */
internal val FREE = ControlState(enabled = true, running = false)

/** One use's switch as the core would answer it; nothing blocks it unless told. */
internal fun useSwitch(
    capability: AccountCapability,
    on: Boolean,
    control: ControlState = FREE,
    blocked: AccountUseBlock? = null,
    status: AccountUseStatus? = null,
    asksProvider: Boolean = false,
    reviews: Boolean = false,
) = AccountUseSwitch(
    capability,
    on,
    control,
    blocked,
    status,
    asksProvider,
    confirmsOff = true,
    reviewsServerChange = reviews,
)

/** A link slot's picker as the core would answer it. */
internal fun linkPicker(
    slot: LinkSlot,
    options: List<LinkedAccount>,
    selected: Int? = null,
    suggested: Int? = null,
) = AccountLinkPicker(slot, options, selected?.toUInt(), suggested?.toUInt(), FREE)

/**
 * A page for [entry], standing in for the core's `account_page`, which no JVM test can call: a free
 * switch per use, drawn on while it is on, unless [uses] says otherwise.
 */
internal fun pageOf(
    entry: AccountEntry,
    uses: List<AccountUseSwitch> = entry.uses.map { useSwitch(it.capability, it.state == CapabilityState.ON) },
    links: List<AccountLinkPicker> = emptyList(),
    remove: ControlState = FREE,
    senderName: AccountSenderName? = null,
    mail: AccountMailSettings? = null,
    servers: AccountServers? = entry.endpoints?.let { AccountServers(form = it, save = FREE) },
) = AccountPage(
    account = entry.id,
    uses = uses,
    links = links,
    senderName = senderName,
    mail = mail,
    servers = servers,
    remove = remove,
    notice = null,
)

/**
 * Setup's pickers, standing in for the core's `setup_link_pickers`: a picker per slot with
 * candidates that nothing fills, its suggestion marked.
 */
internal fun setupPickers(entry: AccountEntry): List<AccountLinkPicker> = listOf(
    Triple(LinkSlot.CALENDAR, entry.linkCandidates.calendar, entry.links.calendar),
    Triple(LinkSlot.CONTACTS, entry.linkCandidates.contacts, entry.links.contacts),
    Triple(LinkSlot.MAIL, entry.linkCandidates.mail, entry.links.mail),
).filter { (_, candidates, current) -> candidates.isNotEmpty() && current == null }
    .map { (slot, candidates, _) ->
        val suggested = candidates.indexOfFirst { it.id in entry.linkCandidates.suggested }
        linkPicker(slot, candidates, suggested = suggested.takeIf { it >= 0 })
    }

/** Settings → Accounts whose opening a page reads [pageOf], standing in for the core's record. */
internal fun openingPages(state: AccountsUiState) = AccountsSettingsUi(
    state,
    onOpenPage = { id ->
        state.page = id
        state.accountPage = id?.let(state::entry)?.let { pageOf(it) }
    },
)
