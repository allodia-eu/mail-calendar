//! The intent dispatch table: one inbound [`Intent`] in, the matching use-case out.
//!
//! Split from `lib.rs` so the runtime's state and lifecycle stay separate from the
//! command surface, and because `lib.rs` sat at the 500-line limit, where every new
//! intent was a merge conflict waiting to happen.

use engine_api::{AccountId, Provider};

use crate::{App, Intent, SearchScope, Surface, scope::Scope, sync::RefreshProgress};

impl<P: Provider> App<P> {
    /// Handles one inbound [`Intent`]. On completion the changed surface is signalled
    /// and the host pulls the new snapshot.
    pub async fn dispatch(&self, intent: Intent) {
        // Feature-adoption counting, gated on consent inside. One classifier over the intent
        // variant; see `telemetry::track_intent` for why that keeps it content-free.
        self.track_intent(&intent);
        match intent {
            Intent::RefreshMail => self.refresh_mail(RefreshProgress::Background).await,
            Intent::SetViewMode(mode) => {
                *self.view_mode.lock().expect("view-mode mutex poisoned") = mode;
                // Persist the grouping so it survives a restart, and signal Settings so the
                // settings surface reflects the new default.
                self.persist_view_mode(mode);
                self.observer.surface_changed(Surface::Settings);
                self.reset_window();
                self.rebuild_snapshot().await;
            }
            Intent::SelectAccount(account) => {
                // The account's own all-mail view, or the unified list. Either way the folder
                // goes with it: a folder belongs to one account, and `Scope` will not hold one
                // without it.
                *self.scope.lock().expect("scope mutex poisoned") = Scope::for_account(
                    account.and_then(|id| AccountId::try_from(id.as_str()).ok()),
                );
                self.reset_window();
                self.rebuild_snapshot().await;
            }
            Intent::SetAccountExpanded { account, expanded } => {
                // No `reset_window`: the list on screen has not changed, only the tree beside
                // it, so scrolling back to the top would be a visible non-sequitur.
                self.set_account_expanded(&account, expanded).await;
            }
            Intent::SelectFolder { folder } => {
                // One write, so the account and the key can never be half-applied: a reader
                // between two writes is how the same key got resolved against the wrong account.
                let (account, key) = (folder.account.clone(), folder.key.clone());
                *self.scope.lock().expect("scope mutex poisoned") = Scope::Folder(folder);
                self.reset_window();
                // The folder is on screen before anything is downloaded. Opening one whose mail
                // isn't synced yet (a custom/untagged folder) costs a provider connection and a
                // download, and awaiting that first left the whole window on the folder the user
                // had just left: for the length of a network round trip, with nothing to say
                // why. The pass below raises the progress bar, which is what a wait the user is
                // aware of looks like (`docs/sync-progress.md`).
                self.rebuild_snapshot().await;
                // Told which account rather than re-reading the selection, so the download
                // cannot follow a selection that has moved on underneath it.
                // Only when something was downloaded: most opens sync nothing, and rebuilding
                // anyway would repaint the list a second time for an identical snapshot.
                if self.ensure_folder_synced(&account, &key).await {
                    self.rebuild_snapshot().await;
                }
            }
            Intent::Search(query) => {
                // An empty query clears search; otherwise a non-blank query is active.
                let query = query.filter(|q| !q.trim().is_empty());
                // Leaving search drops the scope filter with it: the next search opens across
                // everything, rather than silently inheriting how the last one was narrowed
                // (a filter the user can no longer see is a filter they will not think of).
                if query.is_none() {
                    *self
                        .search_scope
                        .lock()
                        .expect("search-scope mutex poisoned") = SearchScope::default();
                }
                *self.search_query.lock().expect("search mutex poisoned") = query;
                self.reset_window();
                self.rebuild_snapshot().await;
            }
            Intent::SetSearchScope(scope) => {
                *self
                    .search_scope
                    .lock()
                    .expect("search-scope mutex poisoned") = scope;
                self.reset_window();
                self.rebuild_snapshot().await;
            }
            Intent::ShowMore => {
                self.grow_window();
                self.rebuild_snapshot().await;
            }
            Intent::OpenMessage { message } => self.open_message(message).await,
            Intent::SubmitMail { to, subject, body } => self.submit_mail(to, subject, body).await,
            Intent::SubmitRichMail {
                from,
                to,
                cc,
                bcc,
                subject,
                document,
                blobs,
            } => {
                self.submit_rich_mail(from, to, cc, bcc, subject, document, blobs)
                    .await;
            }
            Intent::RefreshCalendar => self.refresh_calendar().await,
            Intent::RefreshContacts => self.refresh_contacts().await,
            Intent::SearchContacts { query } => self.search_contacts(query).await,
            Intent::CreateContact {
                account,
                address_book,
                edit,
            } => self.create_contact(account, address_book, edit).await,
            Intent::UpdateContact {
                person,
                account,
                card,
                edit,
            } => self.update_contact(person, account, card, edit).await,
            // The mail-mutation handlers report whether the edit applied, for the agent adapter
            // (`mail_ops::result`). An intent stays fire-and-forget: the interactive surface
            // learns the outcome from the optimistic hide being undone and the re-sync, not from
            // a return value, so the result is deliberately discarded here.
            Intent::MarkRead { message, read } => {
                let _ = self.mark_read(message, read).await;
            }
            Intent::SetFlagged { message, flagged } => {
                let _ = self.set_flagged(message, flagged).await;
            }
            Intent::Delete { message } => {
                let _ = self.delete(message).await;
            }
            Intent::PermanentlyDelete { message } => {
                let _ = self.permanently_delete(message).await;
            }
            Intent::Archive { message } => {
                let _ = self.archive(message).await;
            }
            Intent::ArchiveThread { thread } => self.archive_thread(thread).await,
            Intent::MarkAsSpam { message } => {
                let _ = self.mark_as_spam(message).await;
            }
            Intent::MarkAsNotSpam { message } => {
                let _ = self.mark_as_not_spam(message).await;
            }
            Intent::SubmitRichReply {
                message,
                from,
                to,
                cc,
                bcc,
                document,
                blobs,
            } => {
                self.submit_rich_reply(message, from, to, cc, bcc, document, blobs)
                    .await;
            }
            Intent::SubmitRichForward {
                message,
                from,
                to,
                cc,
                bcc,
                document,
                blobs,
            } => {
                self.submit_rich_forward(message, from, to, cc, bcc, document, blobs)
                    .await;
            }
            Intent::CreateEvent {
                title,
                start,
                end,
                account,
                calendar,
                all_day,
                timezone,
                notes,
                location,
                recurrence,
            } => {
                self.create_event(
                    title, start, end, account, calendar, all_day, timezone, notes, location,
                    recurrence,
                )
                .await;
            }
            Intent::UpdateEvent { event, edit } => {
                // Not `let _ =`. The write is not durable yet (no outbox drainer), so a
                // failure here means the user's edit did not happen. `update_event` already
                // drives `CalendarWriteStatus` to `Failed` for the host to surface; logging
                // it as well is the least this can do until the write becomes durable.
                if let Err(err) = self.update_event(&event, &edit).await {
                    log::error!("update_event: the edit was not saved: {err}");
                }
            }
            Intent::MoveEvent { event, drag } => {
                // As with `UpdateEvent`: not durable yet, so a failure means the drag did not
                // happen. `update_event` underneath has already driven `CalendarWriteStatus`
                // to `Failed` for the host to surface; except when the refusal came *before*
                // the write (an event that is not ours to move), which no client that honours
                // `can_move` can reach.
                if let Err(err) = self.move_event(&event, &drag).await {
                    log::error!("move_event: the drag was not saved: {err}");
                }
            }
            Intent::RespondToInvitation {
                message,
                response,
                comment,
                notify_organizer,
                reply_subject,
            } => {
                // As with `UpdateEvent`: the write drives `CalendarWriteStatus` to `Failed`
                // for the host to surface, and logging the reason is the least this can do
                // until the write becomes durable. The reason names no addresses and no
                // meeting: the errors this returns are all about *shape*, never content.
                if let Err(err) = self
                    .respond_to_invitation(
                        &message,
                        response,
                        comment,
                        notify_organizer,
                        reply_subject,
                    )
                    .await
                {
                    log::error!("respond_to_invitation: the answer was not sent: {err}");
                }
            }
            Intent::AnswerReplyPrompt {
                send,
                remember,
                reply_subject,
            } => {
                // The same treatment as the RSVP above: the failure a user needs to know about
                // is "the organiser still has not been told", and the reason names no address
                // and no meeting.
                if let Err(err) = self
                    .answer_reply_prompt(send, remember, reply_subject)
                    .await
                {
                    log::error!("answer_reply_prompt: the reply was not emailed: {err}");
                }
            }
            Intent::RetryUnfiledCopy => {
                self.retry_unfiled_copy().await;
            }
            Intent::DismissUnfiledCopy => self.dismiss_unfiled_copy(),
            Intent::DeleteEvent { event, occurrence } => {
                self.delete_event(event, occurrence).await;
            }
            Intent::ReportNetworkReachable(reachable) => {
                self.report_network_reachable(reachable).await;
            }
            Intent::ReportDeviceTimeZone(id) => self.report_device_timezone(&id),
            Intent::SetTimeZone(id) => self.set_timezone(id).await,
            Intent::AcceptTimeZoneChange => self.resolve_timezone_change(true).await,
            Intent::DismissTimeZoneChange => self.resolve_timezone_change(false).await,
            Intent::SetQuoteStyle(style) => self.set_default_quote_style(style).await,
            Intent::SetQuoteStylePerMessage(per_message) => {
                self.set_quote_style_per_message(per_message).await;
            }
            Intent::SetDefaultSendAccount(account) => {
                self.set_default_send_account(account).await;
            }
            Intent::SetSwipeAction { direction, action } => {
                self.set_swipe_action(direction, action).await;
            }
        }
    }
}
