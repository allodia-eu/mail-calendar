// Snapshot entries for the Settings → Accounts suites: an account as the core lists it, built
// without the cdylib, which no JVM test loads.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountEndpoints
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.AccountLinksView
import uniffi.mailcal_bindings.AccountUse
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.ConnectionSecurity
import uniffi.mailcal_bindings.LinkCandidates
import uniffi.mailcal_bindings.LinkedAccount
import uniffi.mailcal_bindings.SignInMethod
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

internal fun linked(id: String) = LinkedAccount(id, id)

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
