// The mail/calendar/contacts tab branch of MainScreen, split out of MainActivity.kt: the running
// app's three-destination scaffold (AppNavScaffold) over the folder drawer, the mailbox list, the
// calendar grid and the contacts screen, each wired to its slice of the core.
package eu.allodia.mailcal

import android.util.Log
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.mailcal_bindings.ContactDetail
import uniffi.mailcal_bindings.EventIntent
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.OutboxIntent

private const val TAG = "Mailcal"

@Composable
internal fun MainActivity.MailboxTabContent(instance: MailcalApp) {
    // The contacts reads below are network-free but not free: each blocks on the core's runtime
    // and lands on the store's connection thread, so a call made while a sync holds it waits for
    // it. Off the UI thread, therefore, unlike the detail read this screen already does inline.
    val scope = rememberCoroutineScope()
    // Every account by its address, mail or not: a contact's book may be one without a mailbox,
    // which the switcher rows leave out (docs/accounts.md rule 8).
    val accountLabels = accounts.associate { it.id to it.email } +
        accountSettings.snapshot.accounts.associate { it.id to it.address }
    Column(modifier = Modifier.fillMaxSize()) {
                          // A launch outage no longer shows a raw error dump here (it overflowed the
                          // status bar): accounts that can't connect are kept, badged unreachable,
                          // and surfaced by the friendly in-scaffold connection banner instead.
                          AppNavScaffold(
                            destination = destination,
                            // Where system back leads once nothing is open over the tab.
                            home = homeDestination,
                            onSelect = { picked ->
                                destination = picked
                                // Each tab paints whatever is already cached first, then kicks its
                                // sync, so switching never shows an empty screen while the network
                                // is consulted.
                                when (picked) {
                                    AppDestination.MAIL -> {}
                                    AppDestination.CALENDAR -> {
                                        events = instance.calendarList().events
                                        instance.dispatch(Intent.RefreshCalendar)
                                    }
                                    AppDestination.CONTACTS -> {
                                        // Clear the core's search first. The screen's own field is
                                        // `remember`ed and dies with the composition, but the query
                                        // lives in the core, so without this, leaving Contacts
                                        // mid-search and coming back shows a filtered list under an
                                        // empty search box, which is the "narrowing the user can no
                                        // longer see" failure docs/search.md exists to prevent.
                                        instance.dispatch(Intent.SearchContacts(""))
                                        contacts = instance.contactList().rows
                                        // Read on entering the tab rather than when the create
                                        // button is pressed, because the answer decides whether
                                        // that button exists at all.
                                        scope.launch {
                                            val targets = withContext(Dispatchers.IO) {
                                                instance.contactTargets()
                                            }
                                            contactWrites.targets =
                                                ContactBookChoice.from(targets, accountLabels)
                                        }
                                        instance.dispatch(Intent.RefreshContacts)
                                    }
                                }
                            },
                        ) {
                          if (destination == AppDestination.CONTACTS) {
                            ContactsScreen(
                                rows = contacts,
                                // The query goes into the CORE, not a filter over the rows on
                                // screen: the core matches name, email, phone, organisation and
                                // title, so every client narrows identically, and a person beyond
                                // the loaded page is still findable.
                                onSearch = { query ->
                                    instance.dispatch(Intent.SearchContacts(query))
                                },
                                // A synchronous local read, no network, so a tap opens at once.
                                detailFor = { id ->
                                    try {
                                        instance.contactDetail(id)
                                    } catch (e: Exception) {
                                        Log.w(TAG, "contact detail lookup failed: ${e.javaClass.simpleName}")
                                        null
                                    }
                                },
                                // The detail sheet names the accounts a merged contact came from.
                                // The core's ids are internal, so map them to the addresses the
                                // user actually recognises.
                                accountLabels = accountLabels,
                                writeLine = contactWrites.line(LocalContext.current),
                                outages = outageLines(OutageUse.CONTACTS, LocalContext.current),
                                onOpenAccount = { id -> openAccountPage(id) },
                                onReviewServerChange = { id -> reviewServerChange(id) },
                                canCreate = contactWrites.targets.isNotEmpty(),
                                onCreate = {
                                    contactWrites.editor =
                                        ContactEditorState.create(contactWrites.targets)
                                },
                                // One editable card opens straight into the form. Several is a
                                // question only the user can answer: a person is several
                                // accounts' cards and an edit writes to exactly one of them
                                // (docs/contacts.md §3).
                                onEdit = { detail ->
                                    val cards = ContactCardChoice.from(
                                        detail.editableCards,
                                        accountLabels,
                                    )
                                    if (cards.size == 1) {
                                        openContactEditor(instance, scope, detail.id, cards[0])
                                    } else if (cards.size > 1) {
                                        contactWrites.cardChoice = cards
                                        contactWrites.person = detail.id
                                    }
                                },
                            )
                            ContactWriteSheets(instance, scope)
                          } else if (destination == AppDestination.CALENDAR) {
                            CalendarScreen(
                                // The grid's page query: synchronous, cheap, and never touching the
                                // store or the network, the screen calls it for the page in view
                                // and for its neighbours, to prefetch the next swipe. The week's
                                // first day is NOT passed: it is a core setting the core applies.
                                pageFor = { from, columns ->
                                    instance.calendarRange(from.toString(), columns.toUInt())
                                },
                                // The month is a DIFFERENT query, not the time grid with more
                                // columns: cells and chips, no hour axis, no overlap solving.
                                monthFor = { anchor -> instance.monthPage(anchor.toString()) },
                                // The core owns which day a week begins on (the user's setting), so
                                // no client can disagree with another about it.
                                weekStartFor = { date ->
                                    parseIsoDate(instance.weekStartDate(date.toString()))
                                },
                                display = displaySettings,
                                calendarVersion = calendarVersion,
                                events = events,
                                writeStatus = calendarWriteStatus,
                                activeZoneId = timeZone?.active,
                                onRefreshCalendar = { instance.dispatch(Intent.RefreshCalendar) },
                                onDeleteEvent = { account, key, occurrence ->
                                    instance.dispatch(Intent.Events(EventIntent.Delete(account, key, occurrence)))
                                },
                                onRespondToEvent = { account, key, response, subject ->
                                    instance.dispatch(
                                        Intent.Events(EventIntent.Respond(account, key, response, subject)),
                                    )
                                },
                                // The editor builds the payload (calendar target, all-day, notes,
                                // device-zone wall clock); we just dispatch it.
                                onCreateEvent = { args ->
                                    instance.dispatch(
                                        Intent.Events(EventIntent.Create(
                                            args.title,
                                            args.start,
                                            args.end,
                                            args.account,
                                            args.calendar,
                                            args.allDay,
                                            args.timezone,
                                            args.notes,
                                            args.location,
                                            args.recurrence,
                                            args.invitees,
                                        )),
                                    )
                                },
                                onUpdateEvent = { args ->
                                    instance.dispatch(
                                        Intent.Events(EventIntent.Update(
                                            args.account,
                                            args.key,
                                            args.title,
                                            args.start,
                                            args.end,
                                            args.notes,
                                            args.location,
                                            args.occurrence,
                                            args.recurrence,
                                            args.timesFromOccurrence,
                                            args.invitees,
                                        )),
                                    )
                                },
                                // A drag on the grid. Deliberately NOT an `EventIntent.Update` with new
                                // times: the client sends how far the hand moved, and the core
                                // applies it to the event's own wall clock, so a meeting in
                                // another zone cannot be re-timed by the zone the grid was drawn
                                // in (`mailcal_account::calendar_drag`).
                                onMoveEvent = { args ->
                                    instance.dispatch(
                                        Intent.Events(EventIntent.Move(
                                            args.account,
                                            args.key,
                                            args.edge,
                                            args.days,
                                            args.minutes,
                                            args.occurrence,
                                        )),
                                    )
                                },
                                // A synchronous detail read for the detail sheet and to prefill the
                                // editor, a local store read, no network.
                                eventDetailFor = { account, key, occurrence ->
                                    instance.eventDetail(account, key, occurrence)
                                },
                                // What a whole-series save would cost the occurrences the user
                                // singled out, asked with the payload about to be dispatched, so
                                // the answer is about this edit rather than the worst one.
                                inviteeSuggestionsFor = recipientSuggestionsFor(instance),
                                seriesWarningFor = { args ->
                                    instance.seriesEditWarning(
                                        args.account,
                                        args.key,
                                        uniffi.mailcal_bindings.ProposedEdit(
                                            title = args.title,
                                            start = args.start,
                                            end = args.end,
                                            notes = args.notes,
                                            location = args.location,
                                            // The real one: a rule change is the edit two of the
                                            // four providers answer by discarding every override,
                                            // so the warning has to be asked knowing about it.
                                            recurrence = args.recurrence,
                                        ),
                                    )
                                },
                                // A created timed event is created in the device's zone, not UTC, so
                                // it reads back the same clock on edit.
                                deviceZoneId = java.time.ZoneId.systemDefault().id,
                                // A settled pinch persists the horizon; the core clamps it.
                                onSetVisibleHours = { hours ->
                                    instance.setCalendarVisibleHours(hours.toUByte())
                                },
                                // The shape the user is reading in, remembered across launches.
                                onSetLayout = { instance.setCalendarLayout(it) },
                                // The calendar manager. Both writes are persisted by the core and
                                // applied at page-read time, so the grid redraws at once, no sync,
                                // no network. The palette comes from the core too: a client cannot
                                // invent a colour, and Allodia Orange is deliberately not in it.
                                palette = calendarPalette,
                                onSetCalendarVisible = { account, calendar, visible ->
                                    instance.setCalendarVisible(account, calendar, visible)
                                },
                                onSetCalendarColor = { account, calendar, hex ->
                                    instance.setCalendarColor(account, calendar, hex)
                                },
                                // The calendar's own menu opens Settings on the Calendar category:
                                // the settings that govern this screen are otherwise three taps
                                // away, from a hub the user has to leave the calendar to reach.
                                onOpenCalendarSettings = {
                                    settingsCategory = SettingsCategory.CALENDAR
                                    showingSettings = true
                                },
                                // A Microsoft account missing the calendar scope prompts here; the
                                // action signs it in again in place to grant it.
                                calendarReauth = calendarReauth,
                                onReconnectCalendar = { id -> signInAgain(id, emptyList()) },
                                outages = outageLines(OutageUse.CALENDAR, LocalContext.current),
                                onOpenAccount = { id -> openAccountPage(id) },
                                onReviewServerChange = { id -> reviewServerChange(id) },
                            )
                          } else {
                            // The folder navigation drawer wraps the mailbox screen so it can
                            // slide in from the left edge. State is remembered at this level so
                            // it persists while the user is on the mailbox (re-entering or
                            // switching accounts closes it via the Settled state machine).
                            val drawerState = rememberDrawerState(DrawerValue.Closed)
                            FolderDrawerScaffold(
                                drawerState = drawerState,
                                accounts = accounts,
                                accountFolders = accountFolders,
                                selectedAccount = selectedAccount,
                                selectedFolder = selectedFolder,
                                unifiedUnread = unifiedUnread,
                                queued = outbox.size,
                                showingOutbox = showingOutbox,
                                onSelectAccount = { id ->
                                    instance.dispatch(Intent.SelectAccount(id))
                                },
                                onSelectFolder = { account, key ->
                                    instance.dispatch(Intent.SelectFolder(account, key))
                                },
                                onSetFolderExpanded = { account, key, expanded ->
                                    instance.dispatch(
                                        Intent.SetFolderExpanded(account, key, expanded),
                                    )
                                },
                                onSetExpanded = { id, expanded ->
                                    instance.dispatch(Intent.SetAccountExpanded(id, expanded))
                                },
                                onShowOutbox = {
                                    instance.dispatch(Intent.Outbox(OutboxIntent.Show))
                                },
                                folderEditing = FolderEditing.of(instance),
                                folderNotice = folderNotice,
                            ) {
                            if (showingOutbox) {
                                // The Outbox replaces the mailbox list rather than covering it:
                                // it holds no stored mail, so none of that screen's chrome (the
                                // account switcher, search, the selection bar, pull-to-refresh)
                                // has anything to act on here.
                                OutboxPane(instance, drawerState)
                            } else {
                            MailboxScreen(
                            rows = rows,
                            sendStatus = sendStatus,
                            // Account switcher: the configured accounts and the selected one
                            // (null = unified all-inboxes). A later account is added from Settings
                            // alone (docs/settings.md).
                            accounts = accounts,
                            selectedAccount = selectedAccount,
                            onSelectAccount = { instance.dispatch(Intent.SelectAccount(it)) },
                            onRemoveAccount = { removeAccount(it) },
                            removalOf = { id ->
                                accountSettings.entry(id)
                                    ?.let { accountRemoval(this@MailboxTabContent, it) }
                            },
                            // The folder navigation drawer state, the hamburger icon opens it.
                            drawerState = drawerState,
                            // The list's scroll position, kept by the activity: opening a message
                            // swaps this whole screen out, so the row the user left has to be
                            // remembered somewhere that outlives it.
                            position = mailbox.list,
                            // Live full-text search: a non-empty query shows ranked results (the
                            // snapshot comes back as flat rows), clearing it returns to the folder
                            // view. It's a local FTS query, no network. Kept by the activity for
                            // the same reason as the position above, and because the core holds
                            // its query just as long.
                            search = mailbox.search,
                            searchHorizon = searchHorizon,
                            currentScopeLabel = currentScopeLabel(
                                ctx = this@MailboxTabContent,
                                accountFolders = accountFolders,
                                selectedAccount = selectedAccount,
                                selectedFolder = selectedFolder,
                            ),
                            onRefresh = {
                                logUiInfo("pull-to-refresh requested")
                                instance.dispatch(Intent.RefreshMail)
                            },
                            // Infinite scroll: the list calls this as it nears the end; showMore
                            // grows the window (guarded) and the core re-projects the next page.
                            onShowMore = { showMore() },
                            // Tapping a message opens its reading view (body fetched + sanitised
                            // by the core), or a draft's composer.
                            onOpen = { opened, draft -> openOrResume(instance, opened, draft) },
                            // Tapping a conversation opens its newest message; the reading screen
                            // shows the others as a strip that opens each on tap.
                            onOpenThread = { thread -> openOrResumeThread(instance, thread) },
                            // What a swipe runs, dispatched as intents through the FFI.
                            onSetFlagged = { account, key, flagged ->
                                instance.dispatch(Intent.SetFlagged(account, key, flagged))
                            },
                            onDelete = { account, key -> instance.dispatch(Intent.Delete(account, key)) },
                            onActOnSelection = { rows, action ->
                                instance.dispatch(Intent.ActOnSelection(rows, action))
                            },
                            onCompose = { origin -> openComposer(instance, origin) },
                            // A row's menu is the core's, asked as it opens (docs/row-actions.md).
                            rowMenu = { rows -> instance.rowMenu(rows) },
                            onRowPick = { pick -> performRowPick(instance, pick) },
                            messageFiling = MessageFiling.of(instance, this@MailboxTabContent),
                            // Swipe actions (per direction) + the archive intent a swipe may run;
                            // both come from the core, which persists the choice.
                            swipe = swipeSettings,
                            onArchive = { account, key -> instance.dispatch(Intent.Archive(account, key)) },
                            // Display-timezone change prompt (an overlay). The zone *selector*
                            // now lives in the Settings screen; the accept/dismiss of a device-zone
                            // change stays here as a prompt. State lives in Rust.
                            timeZone = timeZone,
                            onAcceptTimeZoneChange = { instance.dispatch(Intent.AcceptTimeZoneChange) },
                            onDismissTimeZoneChange = { instance.dispatch(Intent.DismissTimeZoneChange) },
                            // Background-download progress bar.
                            syncProgress = syncProgress,
                            emptyReason = emptyReason,
                            // Connectivity: the offline banner + per-account outage badges + the
                            // friendly connection-issues banner (names affected accounts, with a
                            // Details action and Try again). Retry re-dials via a refresh.
                            offline = connectivity?.offline ?: false,
                            unreachableAccounts = connectivity?.unreachableAccounts ?: emptyList(),
                            connectionIssues = connectionIssues,
                            // A Microsoft account whose grant lacks the mail write/send scopes: a
                            // "reconnect to send and manage mail" banner whose action signs it in
                            // again in place (same flow as the calendar prompt).
                            mailReauth = mailReauth,
                            onReconnectMail = { id -> signInAgain(id, emptyList()) },
                            signInExpired = signInExpired,
                            onOpenAccountPage = { id -> openAccountPage(id) },
                            onSignInExpired = { account ->
                                when (AccountSignInRules.expiredRemedy(account.provider)) {
                                    // In place: the account keeps its id, settings and mail, where
                                    // a first sign-in would add it again (provider-oauth.md rule 11).
                                    ExpiredRemedy.PROVIDER_CONSENT ->
                                        signInAgain(account.id, emptyList())
                                    // Addressed to the account id, not the address: the core
                                    // re-authorises this account's own stored grant.
                                    ExpiredRemedy.JMAP_REAUTH ->
                                        reconnectJmap(accountId = account.id)
                                    // From the grant it holds, as its page signs it in again.
                                    ExpiredRemedy.ACCOUNT_SIGN_IN ->
                                        startAccountSignIn(account.id, switching = false)
                                    // The banner offers Settings instead of this button.
                                    ExpiredRemedy.SETTINGS -> openAccountPage(account.id)
                                }
                            },
                            // Open the unified Settings screen (grouping, language, time zone,
                            // fetch depth, sync behaviour, quote style, reset).
                            onOpenSettings = {
                                settingsCategory = null
                                showingSettings = true
                            },
                            )
                            }
                            // A mail link or a share opens its composer from here, so neither
                            // opens over the welcome or setup screens.
                            LinkAndShareComposers(instance)
                            } // FolderDrawerScaffold
                          }
                          }
    }
}
