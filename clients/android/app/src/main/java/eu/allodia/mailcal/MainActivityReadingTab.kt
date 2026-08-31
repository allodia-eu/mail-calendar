// The reading-view branch of MainScreen, split out of MainActivity.kt: the ReadingScreen call
// alone carries every mail action a read message offers (reply/forward, archive/delete,
// attachments, invitation response, recipient autosuggest).
package eu.allodia.mailcal

import android.util.Log
import androidx.compose.runtime.Composable
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.MailcalException

private const val TAG = "Mailcal"

// A message is open: show its reading view over the list.
@Composable
internal fun MainActivity.ReadingTabContent(instance: MailcalApp, opened: OpenedMessage) {
    ReadingScreen(
                            message = opened,
                            reading = reading,
                            conversation = openedConversation,
                            activeZoneId = timeZone?.active,
                            quoteSettings = quoteSettings,
                            calendarWriteStatus = calendarWriteStatus,
                            onRespondToInvitation = {
                                account, key, response, comment, notify, replySubject ->
                                instance.dispatch(
                                    Intent.RespondToInvitation(
                                        account,
                                        key,
                                        response,
                                        comment,
                                        notify,
                                        replySubject,
                                    )
                                )
                            },
                            onBack = {
                                openedMessage = null
                                openedConversation = null
                            },
                            // Retry a failed fetch: clear the body (spinner) and re-open.
                            onRetry = {
                                reading = null
                                instance.dispatch(Intent.OpenMessage(opened.account, opened.key))
                            },
                            // Open another message of the same conversation from the strip, keeping
                            // the conversation context so the strip persists (accordion, one body
                            // at a time).
                            onOpenThreadMessage = { next -> openMessage(instance, next, openedConversation) },
                            // Reply/reply-all/forward open the SAME shared rich composer as the
                            // list rows; the user-confirmed recipients ride the submit, and the
                            // core derives the Re:/Fwd: subject + threading from the original.
                            // `from` is the account picked in the composer's From dropdown; the
                            // core sends as, and through, it, defaulting to `account`.
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
                            // The From dropdown lists every configured account.
                            accounts = accounts,
                            onSaveAttachment = { account, key, attachmentId, destinationPath ->
                                try {
                                    instance.saveAttachment(account, key, attachmentId, destinationPath)
                                    true
                                } catch (e: MailcalException) {
                                    Log.w(TAG, "attachment save failed: ${e.javaClass.simpleName}")
                                    false
                                }
                            },
                            // Archive/delete move the message out of the folder; the core hides
                            // the row optimistically, and the screen pops back to the list.
                            onArchive = { account, key -> instance.dispatch(Intent.Archive(account, key)) },
                            onDelete = { account, key -> instance.dispatch(Intent.Delete(account, key)) },
                            // Screenshot only: MAILCAL_SHOWCASE_SCREEN=reply opened this message, so
                            // open its reply composer too, pre-filled with the sample reply text.
                            initialComposing = RichComposeMode.Reply.takeIf {
                                ShowcaseMode.isOn(this@ReadingTabContent) &&
                                    ShowcaseMode.screen(this@ReadingTabContent) == ShowcaseScreen.REPLY
                            },
                            composerInitialText = ShowcaseMode.replyText(
                                this@ReadingTabContent, opened.account, opened.key,
                            ),
                        )
}
