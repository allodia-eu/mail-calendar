#!/usr/bin/env bash
# The Linux UI suite's mail legs: reading, replying, forwarding, printing, search and the actions a
# message and a conversation offer. Sourced by test-linux-ui.sh.

# Open <subject> from a search across all mail: a message row opens through the row itself, whose
# activatable widget is the button that opens it.
open_mail_message() { # <subject>
  ui_press text=Mail role=toggle-button
  search_for "$1"
  ui_press role=list-item text="$1" enabled=true timeoutMs=45000
  [[ "$UI_REPLY" != *'"type": "AdwExpanderRow"'* ]] || die "'$1' is listed as a conversation"
}

# A conversation row's own Open button: the row is an expander, which a press only opens up.
open_conversation() { # <subject>
  ui_within press role=list-item text="$1" type=AdwExpanderRow -- id=row-open index=0
}

# The reading pane's header, which holds the open message's own actions.
reading_header() {
  ui_one type=AdwHeaderBar has='{"text":"Forward","role":"button"}'
}

# Opened by searching the Inboxes alone: each run's reply to this fixture is filed in Sent and
# shares its thread, so across all mail the match is one conversation named after the newest reply.
# Within the Inboxes the fixture is the newest match, so the row is named after it. Until a run has
# replied, the row is a message of its own, whose button carries the subject. After, it is a
# conversation, whose "Open" expands it onto its newest message, a previous run's reply
# (docs/row-actions.md, rule 2), and the fixture is then its own row inside it, named after its
# sender.
leg_reply() {
  ui_press text=Mail role=toggle-button
  search_for "$REMOTE_SUBJECT"
  ui_press text=Inboxes role=toggle-button
  # The row, whichever it is: an expander's own header row says the same words and is left out as
  # its echo, so a conversation is found as the expander.
  ui_one role=list-item text="$REMOTE_SUBJECT" enabled=true timeoutMs=45000
  local row="$UI_H"
  if [[ "$UI_REPLY" == *'"type": "AdwExpanderRow"'* ]]; then
    # Unfolded rather than opened: Open would open the newest reply first, which quotes the
    # remote image and raises the same banner, and the fixture's open after it would undo an
    # opt-in pressed in between. Unfolded, the thread shows its messages, each with an Open.
    ui activate h="$row"
    ui_wait under="$row" id=row-open index=1
    # The fixture's own row inside the thread is the last row naming its sender: the
    # conversation, and the header row it draws itself with, come first and name it too.
    ui_wait under="$row" type=AdwActionRow text="$REMOTE_SENDER_NAME"
    [[ "$UI_REPLY" =~ .*\"h\":\ ([0-9]+) ]] || die "no row in the conversation names $REMOTE_SENDER_NAME"
    ui activate h="${BASH_REMATCH[1]}"
  else
    ui activate h="$row"
  fi
  ui_wait text=Reply role=button enabled=true timeoutMs=30000
  ui_wait text="Remote images are blocked to protect your privacy." timeoutMs=30000
  capture reading-blocked

  # Choosing the row selects it as well, and the selection opens the message again a moment later,
  # which starts it blocked again; an opt-in pressed before that would be undone.
  ui settle
  ui_press text="Load images" role=button
  # The banner's button stays where it was while the banner folds away; the banner is what goes.
  ui_gone text="Remote images are blocked to protect your privacy."
  capture reading-opted-in
  ui_press text=Reply role=button
  # AT-SPI: this static name is replaced with the localised value by setComposerLabels, so seeing
  # it on the editable node proves WebKit exposed the loaded editor to assistive technology.
  atspi wait --name "Write your message" --showing --timeout 30
  capture reply

  # A reply's derived recipient is *finished*, so it is a pill with its own remove control and
  # nothing is left half-typed in the input; the failure this catches is a field that looks like it
  # dropped the person it is in fact holding. AT-SPI: the address is the fixture's own sender, and
  # what is asserted is that the remove control's spoken name says so rather than a bare "Remove".
  atspi wait --name "$REMOTE_SENDER: Remove recipient" --role "push button" \
    --enabled --showing --timeout 20

  # Subject is a caption in one grid cell and an entry in the next, which relates them to the eye
  # and to nothing else: the entry reached assistive technology with an empty name, so someone
  # tabbing into it was told only that it was a text field. Nothing on the screen changes when that
  # breaks, and the GTK bindings expose no getter for an accessible name, so AT-SPI is the only
  # thing that can see it.
  atspi wait --name "Subject" --role text --enabled --showing --timeout 20
  capture reply-recipients

  # The core owns a 2.5-second terminal status; the wait is asked on the open connection straight
  # after the press.
  ui_press text=Send role=button
  ui_wait text="Message sent" timeoutMs=45000
  capture sent
  # Clearing the query puts the scope back to all mail, which is where the forward leg searches.
  search_for ""
}

# Forwarding carries the original's files, drawn as tiles above the message (docs/sending.md ->
# "What a forward carries"; docs/composer-layout.md). What only a live run shows is that the
# staging round trip through the core reaches the list the user can act on, rather than attaching
# the files somewhere the composer never draws.
leg_forward() {
  open_mail_message "$ATTACHMENT_SUBJECT"
  ui_press text=Forward role=button
  ui_wait id=composer-editor timeoutMs=30000
  # The tile, and the control that takes it off again. A forward that carried the file while showing
  # nothing is the shape this leg exists to catch, and it looks identical from outside.
  ui_within wait id=composer-file text="$ATTACHMENT_FILE" enabled=true timeoutMs=30000 -- \
    text=Remove role=button enabled=true
  capture forward-attachments

  # Abandoning an untouched forward asks nothing: its files are still in the mailbox. The absence
  # of the attachment tile is what says the navigation went through; a guard measuring the staged
  # files against zero would have stopped it with a modal and left the tile on screen.
  open_mail_message "$PLAIN_SUBJECT"
  ui_gone id=composer-file text="$ATTACHMENT_FILE" timeoutMs=30000
  ui_gone type=GtkWindow text="Discard draft?"

  # Taking one off is a decision about what goes out, so the same navigation now keeps the composer
  # in Drafts, still without asking (docs/drafts.md, "Leaving a composer"). Both halves are needed:
  # the first alone passes just as well with the save switched off entirely.
  open_mail_message "$ATTACHMENT_SUBJECT"
  ui_press text=Forward role=button
  ui_within press id=composer-file text="$ATTACHMENT_FILE" enabled=true timeoutMs=30000 -- \
    text=Remove role=button
  ui_gone id=composer-file text="$ATTACHMENT_FILE"
  open_mail_message "$PLAIN_SUBJECT"
  ui_gone id=composer-editor timeoutMs=30000
  ui_gone type=GtkWindow text="Discard draft?" timeoutMs=5000
  # Out of the search, into Drafts, where the forward it left behind is listed.
  search_for ""
  ui_press role=list-item text=Drafts index=0
  ui_wait role=list-item text="Fwd: $ATTACHMENT_SUBJECT" enabled=true timeoutMs=45000
  capture forward-left-in-drafts

  # Reopened, it has a copy in Drafts to lose, so Discard asks; Keep editing changes nothing, and
  # Discard takes the copy off the server. A saved forward keeps its conversation, so its Drafts row
  # is a conversation's, opened by its Open button. The question carries a Discard of its own beside
  # the composer's, hence the window.
  open_conversation "Fwd: $ATTACHMENT_SUBJECT"
  ui_wait id=composer-editor timeoutMs=30000
  ui_press text=Discard role=button
  ui_wait type=GtkWindow text="Discard draft?" timeoutMs=30000
  capture forward-discard-prompt
  ui_press text="Keep editing" role=button
  ui_gone type=GtkWindow text="Discard draft?"
  ui_wait id=composer-editor
  ui_press text=Discard role=button
  ui_wait type=GtkWindow text="Discard draft?" timeoutMs=30000
  ui_press text=Discard role=button window="Discard draft?"
  ui_gone type=GtkWindow text="Discard draft?"
  ui_gone role=list-item text="Fwd: $ATTACHMENT_SUBJECT" timeoutMs=45000
  # Back to All Inboxes, the first Inbox row: a search's scope follows the open folder, and the
  # legs below expect the inboxes'.
  ui_press role=list-item text=Inbox index=0
}

# Printing (docs/reading-actions.md, "Printing a message"). Inside the sandbox WebKitGTK prints only
# through the desktop portal, which the fixture accepts without a dialog, keeping the PDF it is
# handed; so what is read back is the page that would have reached the printer. Print is pressed
# twice, and the fixture answers the two dialogs together: the portal's dialog leaves the window
# usable, so a second print can start while the first is still asking, and both pages have to
# arrive.
leg_print() {
  open_mail_message "$PLAIN_SUBJECT"
  ui_wait text="$PLAIN_SUBJECT" role=label index=1 timeoutMs=30000
  reading_header
  local header="$UI_H" printed printed_text
  for _ in 1 2; do
    ui_press under="$header" text="More actions" role=button
    ui_press text=Print… role=button
  done
  local prints=0
  for _ in {1..200}; do
    # The portal's capture exists from its first call; until then, no print has reached it.
    prints="$(grep -c '"method": "Print"' "$PORTAL_CAPTURE" 2>/dev/null || true)"
    (( ${prints:-0} >= 2 )) && break
    sleep 0.05
  done
  (( ${prints:-0} >= 2 )) || die "two prints did not both reach the print portal"
  printed="$(sed -n 's/.*"file": "\([^"]*\)", "method": "Print".*/\1/p' "$PORTAL_CAPTURE" | tail -1)"
  printed_text="$(pdftotext -layout "$printed" -)" || die "the printed page is not a PDF"
  for expected in "$PLAIN_SUBJECT" "Alice Tester" "Bob Tester" "Baseline plaintext message"; do
    grep -Fq "$expected" <<<"$printed_text" || die "the printed page does not carry '$expected'"
  done
  search_for ""
  ui_press role=list-item text=Inbox index=0
}

# Search: that the core's narrowing reaches the screen, that the two controls docs/search.md
# requires of a client are there, and that clearing the field puts the unsearched list back.
#
# The query is a thread, which is what shows that the results carry the list's grouping
# (`docs/search.md`, rule 9): the two messages that matched arrive as one conversation row carrying
# both, not as a row each. The row is named after the newest match, and the seed delivers the root
# and the reply in the same second, so either can name it: it is found by the words both share.
# The last message the notification leg delivered matches nothing in it and must leave; being this
# run's newest, it is the top row of the unsearched list however much mail earlier runs left.
leg_search() {
  search_for kickoff
  # AT-SPI: the count badge is how the row tells a screen reader it carries two messages.
  atspi wait --name-substring "Project kickoff" --role "list item" \
    --contains-name "2 messages" --enabled --showing --timeout 30
  ui_gone role=list-item text="$NOTIFICATION_AFTER" timeoutMs=30000
  # Rule 8: how far back it looked; the harness account's depth is the shipping default; and the
  # route to the setting that decides it. A statement the user cannot act on is half the value.
  ui_wait text="Searching the last 3 months"
  ui_wait text=Change role=button enabled=true
  # Rule 4: the two-way filter, its narrowing side naming the view the search was opened from, the
  # unified inbox here, which is every account's Inbox rather than any one folder.
  ui_wait text="All mail" role=toggle-button enabled=true
  ui_press text=Inboxes role=toggle-button
  ui_wait role=list-item textLike="Project kickoff" enabled=true timeoutMs=30000
  capture search-results

  # Rules 5 and 6: clearing the field leaves search; the folder view returns, threaded again, and
  # takes the filter with it, so no narrowing is left claiming to be in force.
  search_for ""
  ui_wait role=list-item text="$NOTIFICATION_AFTER" enabled=true timeoutMs=30000
  ui_gone text="All mail" role=toggle-button
  capture search-cleared
}

# Mail actions. The dispatches themselves are unit-tested; what only a live run can show is that the
# core answered; the menu offering the opposite verb the next time it opens; and that archiving
# from the reading pane advances it to the next message instead of emptying it (README ->
# "Archive/delete advances the reading pane"). The message is delivered for this run and archived
# by it, so nothing in the seeded mailbox is touched.
#
# It is opened from the Inbox itself rather than from a search: the newest delivery is the list's
# top row, the Inbox is where archiving takes it from, and the pane has a next message to move on
# to, which a search holding this one message would not.
leg_mail_actions() {
  "$REPO_ROOT/scripts/dev/harness.sh" deliver --subject "$MAIL_ACTION_SUBJECT" >/dev/null
  ui_press text=Mail role=toggle-button
  search_for ""
  ui_press role=list-item text=Inbox index=0
  ui_press text=Sync role=button
  local row=(role=list-item text="$MAIL_ACTION_SUBJECT")
  ui_press "${row[@]}" timeoutMs=60000
  ui_wait text="$MAIL_ACTION_SUBJECT" role=label index=1 timeoutMs=30000
  # Opening it marks it read, and the row is drawn again once the core has done so: a menu opened
  # before then belongs to the row being replaced.
  ui settle

  # Opening it read it, so the menu on its own row offers the way back rather than "Mark as read";
  # the round trip, from a dispatch through the core to the snapshot the row is rebuilt from. Every
  # step is scoped to the row: the command bar has a Mark as read and a Flag of its own, which act
  # on the selection rather than on this message.
  ui_within press "${row[@]}" -- text="More actions" role=button
  ui_within wait "${row[@]}" -- text="Mark as unread" role=button enabled=true
  ui_within gone "${row[@]}" -- text="Mark as read"
  capture mail-actions-menu
  ui_within press "${row[@]}" -- text=Flag role=button
  # Wait for the row to carry the flag before reopening its menu: the list reconciles in place, so
  # a menu opened mid-rebuild belongs to the widget being replaced and answers nothing.
  ui_wait "${row[@]}" has='{"text":"Flagged"}' timeoutMs=30000
  ui_within press "${row[@]}" -- text="More actions" role=button
  ui_within press "${row[@]}" -- text=Unflag role=button
  ui_gone "${row[@]}" has='{"text":"Flagged"}' timeoutMs=30000

  # Archive from the pane the message is open in: the pane's own action, not the folder pane's
  # Archive row nor the command bar's, which acts on the selection. The row goes, and the pane keeps
  # a message: an empty one would leave "Select a message to read." and no Reply to press, which is
  # exactly the state this rule exists to prevent.
  reading_header
  ui_press under="$UI_H" text=Archive role=button
  ui_gone role=list-item text="$MAIL_ACTION_SUBJECT" timeoutMs=45000
  ui_gone text="Select a message to read."
  ui_wait text=Reply role=button enabled=true
  capture mail-actions-archived
}

# A conversation's own menu (docs/row-actions.md). The items are the core's, unit-tested there; what
# a live run shows is the round trip: Flag reaches the newest message received, the conversation
# row draws the flag that write left, and its menu then offers Unflag, which takes every flag off.
# Both writes undo each other, so the seed is as the run found it.
leg_conversation_menu() {
  ui_press text=Mail role=toggle-button
  search_for ""
  ui_press role=list-item text=Inbox index=0
  local row=(role=list-item text="$CONVERSATION_SUBJECT")
  ui_wait "${row[@]}" enabled=true timeoutMs=60000
  # The conversation's own menu is the first in its row; each message's follows it.
  ui_within press "${row[@]}" -- text="More actions" role=button index=0
  for item in "Reply" "Forward" "Mark as Spam" "Move to folder…"; do
    ui_within wait "${row[@]}" -- text="$item" role=button enabled=true
  done
  capture conversation-menu
  ui_within press "${row[@]}" -- text=Flag role=button
  ui_wait "${row[@]}" has='{"text":"Flagged"}' timeoutMs=30000
  ui_within press "${row[@]}" -- text="More actions" role=button index=0
  ui_within press "${row[@]}" -- text=Unflag role=button
  ui_gone "${row[@]}" has='{"text":"Flagged"}' timeoutMs=30000
}

# A draft resumes wherever it is listed (docs/drafts.md): opening a conversation in the Inbox whose
# newest message is a draft opens a composer, not the reading pane. Left untouched, the navigation
# away keeps the draft without a question or a write.
leg_draft_resume() {
  ui_press text=Mail role=toggle-button
  search_for ""
  ui_press role=list-item text=Inbox index=0
  ui_wait role=list-item text="$DRAFT_CONVERSATION_SUBJECT" timeoutMs=60000
  open_conversation "$DRAFT_CONVERSATION_SUBJECT"
  ui_wait id=composer-editor timeoutMs=30000
  capture draft-resumed-from-inbox
  open_mail_message "$PLAIN_SUBJECT"
  ui_gone id=composer-editor timeoutMs=30000
  ui_gone type=GtkWindow text="Discard draft?" timeoutMs=5000
}
