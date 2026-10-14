// The reading-view branch of MainScreen, split out of MainActivity.kt: the ReadingScreen call
// alone carries every mail action a read message offers (reply/forward, archive/delete,
// attachments, invitation response).
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
                            // Reply, reply all and forward open the one composer every route
                            // opens, on this message (MainActivityComposer.kt).
                            onCompose = { origin -> openComposer(instance, origin) },
                            onSaveAttachment = { account, key, attachmentId, destinationPath ->
                                try {
                                    instance.saveAttachment(account, key, attachmentId, destinationPath)
                                    true
                                } catch (e: MailcalException) {
                                    Log.w(TAG, "attachment save failed: ${e.javaClass.simpleName}")
                                    false
                                }
                            },
                            onExportMessage = { account, key, destinationPath ->
                                try {
                                    instance.saveMessageSource(account, key, destinationPath)
                                    true
                                } catch (e: MailcalException) {
                                    Log.w(TAG, "message export failed: ${e.javaClass.simpleName}")
                                    false
                                }
                            },
                            // Archive/delete move the message out of the folder; the core hides
                            // the row optimistically, and the screen pops back to the list.
                            onArchive = { account, key -> instance.dispatch(Intent.Archive(account, key)) },
                            onDelete = { account, key -> instance.dispatch(Intent.Delete(account, key)) },
                            // The sun/moon button: the preference Settings → Reading also shows.
                            // Applied here as well, so the open message re-renders on the press
                            // rather than on the SETTINGS pull that follows it.
                            darkMessages = displaySettings.darkMessageBackground,
                            onSetDarkMessages = { dark ->
                                displaySettings = displaySettings.copy(darkMessageBackground = dark)
                                instance.setDarkMessageBackground(dark)
                            },
                            // Screenshot only: MAILCAL_SHOWCASE_SCREEN=reply opened this message, so
                            // open its reply composer too; `openComposer` lays the sample reply
                            // text over it.
                            opensReplyOnShow = ShowcaseMode.isOn(this@ReadingTabContent) &&
                                ShowcaseMode.screen(this@ReadingTabContent) == ShowcaseScreen.REPLY,
                        )
}
