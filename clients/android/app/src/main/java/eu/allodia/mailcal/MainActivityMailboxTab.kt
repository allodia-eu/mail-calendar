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
import androidx.compose.ui.Modifier
import uniffi.mailcal_bindings.AccountProvider
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.MailcalException

private const val TAG = "Mailcal"

@Composable
internal fun MainActivity.MailboxTabContent(instance: MailcalApp) {
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
                                accountLabels = accounts.associate { it.id to it.email },
                            )
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
                                    instance.dispatch(Intent.DeleteEvent(account, key, occurrence))
                                },
                                // The editor builds the payload (calendar target, all-day, notes,
                                // device-zone wall clock); we just dispatch it.
                                onCreateEvent = { args ->
                                    instance.dispatch(
                                        Intent.CreateEvent(
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
                                        ),
                                    )
                                },
                                onUpdateEvent = { args ->
                                    instance.dispatch(
                                        Intent.UpdateEvent(
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
                                        ),
                                    )
                                },
                                // A drag on the grid. Deliberately NOT an `UpdateEvent` with new
                                // times: the client sends how far the hand moved, and the core
                                // applies it to the event's own wall clock, so a meeting in
                                // another zone cannot be re-timed by the zone the grid was drawn
                                // in (`mailcal_account::calendar_drag`).
                                onMoveEvent = { args ->
                                    instance.dispatch(
                                        Intent.MoveEvent(
                                            args.account,
                                            args.key,
                                            args.edge,
                                            args.days,
                                            args.minutes,
                                            args.occurrence,
                                        ),
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
                                // action re-runs its sign-in (login_hint = its address) to grant it.
                                calendarReauthEmails = calendarReauthEmails,
                                onReconnectCalendar = { email -> signInWithMicrosoft(loginHint = email) },
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
                                onSelectAccount = { id ->
                                    instance.dispatch(Intent.SelectAccount(id))
                                },
                                onSelectFolder = { account, key ->
                                    instance.dispatch(Intent.SelectFolder(account, key))
                                },
                                onSetExpanded = { id, expanded ->
                                    instance.dispatch(Intent.SetAccountExpanded(id, expanded))
                                },
                            ) {
                            MailboxScreen(
                            rows = rows,
                            sendStatus = sendStatus,
                            // Account switcher: the configured accounts, the selected one
                            // (null = unified all-inboxes), and the add-account affordance.
                            accounts = accounts,
                            selectedAccount = selectedAccount,
                            onSelectAccount = { instance.dispatch(Intent.SelectAccount(it)) },
                            onAddAccount = {
                                addError = null
                                addingAccount = true
                            },
                            onRemoveAccount = { removeAccount(it) },
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
                            // Tapping a message opens its reading view (body fetched + sanitized
                            // by the core).
                            onOpen = { opened -> openMessage(instance, opened) },
                            // Tapping a conversation opens its latest message; the reading screen
                            // shows the older messages as a strip that opens each on tap.
                            onOpenThread = { thread -> openThread(instance, thread) },
                            // Per-message mail actions, dispatched as intents through the FFI.
                            onSetRead = { account, key, read -> instance.dispatch(Intent.MarkRead(account, key, read)) },
                            onSetFlagged = { account, key, flagged ->
                                instance.dispatch(Intent.SetFlagged(account, key, flagged))
                            },
                            onDelete = { account, key -> instance.dispatch(Intent.Delete(account, key)) },
                            onPermanentlyDelete = { account, key ->
                                instance.dispatch(Intent.PermanentlyDelete(account, key))
                            },
                            // Derive whether the current folder is the Junk/Spam folder, used to
                            // show "Mark as Not Spam" instead of "Mark as Spam" in the overflow.
                            inJunkFolder = accountFolders
                                .firstOrNull { it.accountId == selectedAccount }
                                ?.folders
                                ?.firstOrNull { it.key == selectedFolder }
                                ?.role == uniffi.mailcal_bindings.FolderRole.JUNK,
                            onMarkAsSpam = { account, key ->
                                instance.dispatch(Intent.MarkAsSpam(account, key))
                            },
                            onMarkAsNotSpam = { account, key ->
                                instance.dispatch(Intent.MarkAsNotSpam(account, key))
                            },
                            onArchiveThread = { account, threadId ->
                                instance.dispatch(Intent.ArchiveThread(account, threadId))
                            },
                            // Reply/reply-all/forward go through the SAME shared rich composer as
                            // new mail: the user-confirmed recipients ride the submit, and the
                            // Rust core derives the Re:/Fwd: subject + threading from the original.
                            onReply = { account, key, from, recipients, documentJson, files ->
                                try {
                                    instance.submitRichReplyWithFiles(account, key, recipients, documentJson, files, from)
                                    true
                                } catch (e: MailcalException) {
                                    Log.w(TAG, "rich reply submit failed: ${e.javaClass.simpleName}")
                                    false
                                }
                            },
                            onForward = { account, key, from, recipients, documentJson, files ->
                                try {
                                    instance.submitRichForwardWithFiles(account, key, recipients, documentJson, files, from)
                                    true
                                } catch (e: MailcalException) {
                                    Log.w(TAG, "rich forward submit failed: ${e.javaClass.simpleName}")
                                    false
                                }
                            },
                            // Pre-fill a reply/reply-all's To/Cc from the core (empty on failure).
                            // Composer autosuggest: ranked addresses for a partially-typed
                            // recipient, drawn from synced contacts AND from people the user has
                            // written to before, so it works on an account with no address book.
                            // A local in-memory read in the core, safe to call per keystroke.
                            suggestionsFor = { query ->
                                try {
                                    instance.recipientSuggestions(query)
                                } catch (e: Exception) {
                                    Log.w(TAG, "recipient suggestions failed: ${e.javaClass.simpleName}")
                                    emptyList()
                                }
                            },
                            // The composer's signature: seeded from the From account's slot for
                            // this mode, re-resolved when From changes, overridable per message.
                            signatures = composerSignatures(instance, signatures?.signatures.orEmpty()),
                            replyRecipients = { account, key, replyAll ->
                                try {
                                    instance.replyRecipients(account, key, replyAll)
                                } catch (e: Exception) {
                                    Log.w(TAG, "reply recipients lookup failed: ${e.javaClass.simpleName}")
                                    null
                                }
                            },
                            onSubmitRich = { from, recipients, subject, documentJson, files ->
                                try {
                                    instance.submitRichMailWithFiles(recipients, subject, documentJson, files, from)
                                    true
                                } catch (e: MailcalException) {
                                    Log.w(TAG, "rich composer submit failed: ${e.javaClass.simpleName}")
                                    false
                                }
                            },
                            // Swipe actions (per direction) + the archive intent a swipe may run;
                            // both come from the core, which persists the choice.
                            swipe = swipeSettings,
                            onArchive = { account, key -> instance.dispatch(Intent.Archive(account, key)) },
                            defaultSendAccount = defaultSendAccount,
                            // Display-timezone change prompt (an overlay). The zone *selector*
                            // now lives in the Settings screen; the accept/dismiss of a device-zone
                            // change stays here as a prompt. State lives in Rust.
                            timeZone = timeZone,
                            onAcceptTimeZoneChange = { instance.dispatch(Intent.AcceptTimeZoneChange) },
                            onDismissTimeZoneChange = { instance.dispatch(Intent.DismissTimeZoneChange) },
                            // Background-download progress bar.
                            syncProgress = syncProgress,
                            // Connectivity: the offline banner + per-account outage badges + the
                            // friendly connection-issues banner (names affected accounts, with a
                            // Details action and Try again). Retry re-dials via a refresh.
                            offline = connectivity?.offline ?: false,
                            unreachableAccounts = connectivity?.unreachableAccounts ?: emptyList(),
                            connectionIssues = connectionIssues,
                            // A Microsoft account whose grant lacks the mail write/send scopes: a
                            // "reconnect to send and manage mail" banner whose action re-runs its
                            // sign-in with the full scope set (same flow as the calendar prompt).
                            mailReauthEmails = mailReauthEmails,
                            onReconnectMail = { email -> signInWithMicrosoft(loginHint = email) },
                            signInExpired = signInExpired,
                            onSignInExpired = { account ->
                                when (account.provider) {
                                    AccountProvider.MICROSOFT ->
                                        signInWithMicrosoft(loginHint = account.email)
                                    AccountProvider.GOOGLE ->
                                        signInWithGoogle(loginHint = account.email)
                                    // Addressed to the account id, not the address: the core
                                    // re-authorises this account's own stored grant.
                                    AccountProvider.JMAP_OAUTH ->
                                        reconnectJmap(accountId = account.id)
                                    // The banner offers no button for these, so this is
                                    // unreachable; kept exhaustive rather than silently ignoring
                                    // a family added later.
                                    else -> Unit
                                }
                            },
                            // Open the unified Settings screen (grouping, language, time zone,
                            // fetch depth, sync behaviour, quote style, reset).
                            onOpenSettings = {
                                settingsCategory = null
                                showingSettings = true
                            },
                            // A mail link (`mailto:`) opens the composer pre-filled; clearing it
                            // when the composer closes is what stops it re-opening on the next
                            // recomposition.
                            mailtoPrefill = pendingMailto,
                            onMailtoConsumed = { pendingMailto = null },
                            )
                            } // FolderDrawerScaffold
                          }
                          }
    }
}
