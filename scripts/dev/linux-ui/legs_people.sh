#!/usr/bin/env bash
# The Linux UI suite's legs about people: contacts, recipient autosuggest, signatures and the
# cross-account merge. Sourced by test-linux-ui.sh.

# Contacts: that the core's narrowing reaches the screen, and that two people who merely share a
# name stay two rows. The fixtures exist for exactly that (docker/stalwart/seed/contacts).
leg_contacts() {
  local refreshes_before refreshes
  refreshes_before="$(grep -c 'refresh_contacts:' "$APP_LOG" || true)"
  ui_press text=Contacts role=toggle-button
  ui_wait role=list-item text="Ahmed El Amrani" enabled=true timeoutMs=45000
  for _ in {1..200}; do
    refreshes="$(grep -c 'refresh_contacts:' "$APP_LOG" || true)"
    (( refreshes > refreshes_before )) && break
    sleep 0.05
  done
  (( refreshes > refreshes_before )) || die "contacts refresh did not settle before search"
  # A section letter is a row of its own, and `#` is the bucket a name starting with a digit files
  # under rather than minting a section of its own.
  ui_wait text="#" role=label
  # `index=1` is the assertion: a *second* row with that name is on screen. The two namesake
  # fixtures share a name and nothing else, and names never join; a merge on name would leave one.
  ui_wait role=list-item text="Jan de Vries" index=1 enabled=true
  capture contacts-list

  local search_set=0
  for _ in {1..3}; do
    search_for Vermeulen
    for _ in {1..40}; do
      if grep -q 'rebuild_contacts: .*query_chars=9' "$APP_LOG"; then
        search_set=1
        break 2
      fi
      sleep 0.05
    done
  done
  [[ "$search_set" == 1 ]] || die "contact search never reached the core"
  ui_wait role=list-item text="Sofie Vermeulen" enabled=true timeoutMs=30000
  ui_gone role=list-item text="Ahmed El Amrani" timeoutMs=30000
  capture contacts-searched

  # The editor, driven up to the point of a write and no further: this suite runs against the shared
  # harness and the app offers no delete, so a create would leave a card behind every run. What it
  # asserts is that the create affordance is offered at all (it is hidden unless the core reports a
  # writable address book), that the form opens empty, and that Save on an empty form REFUSES rather
  # than closing over a card nobody could find again. The write itself is gated in `mailcal-app`'s
  # contacts suite against a real engine.
  search_for ""
  ui_press text="New contact" role=button
  ui_wait text="First name" type=AdwEntryRow
  ui_press text=Save role=button window="New contact"
  # Still open, and saying why: the message is the client's, because the core has no locale.
  ui_wait text="Enter a name, an organisation or an email address."
  capture contacts-editor
  ui_press text=Cancel role=button window="New contact"
  ui_gone text="First name" type=AdwEntryRow
}

# Autosuggest reads the same people index the contacts leg has just synced, so a seeded contact is
# a deterministic query. The list arriving at all is the assertion, and a popover is in its own
# surface, where a window capture cannot see it.
leg_autosuggest() {
  ui_press text=Mail role=toggle-button
  ui_press text="New Mail" role=button
  # AT-SPI: the caption beside the field carries the same accessible name on purpose, which is what
  # tells a screen reader To from Bcc, and only the entry takes text.
  atspi wait --name "To" --role text --enabled --showing --timeout 30
  # Settings opens over a composer and stays until closed, and the composer is still there after.
  open_settings
  close_settings
  ui_wait id=compose-to enabled=true
  ui_fill id=compose-to value=sofie
  ui_one id=recipient-suggestions timeoutMs=30000
  # A *saved contact*, by name: that it is the name rather than the address is what proves the
  # people index reached the dropdown rather than sent-mail history alone.
  ui_wait under="$UI_H" role=list-item text="Sofie Vermeulen"
  capture compose-autosuggest
  # An empty token closes the list rather than offering everyone the user has ever written to.
  ui_fill id=compose-to value=""
  ui_gone id=recipient-suggestions
  # To is back to what the composer opened with, so nothing was written: no question.
  ui_press text=Discard role=button
  ui_gone id=composer-editor
}

# Signatures: the Settings category, and the round trip through the core. Settings opens on this
# category because MAILCAL_SHOWCASE_SCREEN says so.
leg_signatures() {
  open_settings
  ui_wait text="New signature" role=button enabled=true timeoutMs=30000
  ui_wait text="You haven't written a signature yet."
  # AT-SPI: both slot pickers read "None" until one is set, so a picker named only by its value
  # says nothing about which slot it is. The name is what a screen reader gets.
  atspi wait --name "For new messages" --role "combo box" --enabled --showing --timeout 20
  atspi wait --name "For replies or forwards" --role "combo box" --enabled --showing --timeout 20
  capture signatures-empty

  ui_press text="New signature" role=button
  ui_wait type=GtkEntry window=Settings enabled=true
  # AT-SPI: the editor's body is a WebKit view whose document never reaches the accessibility bus,
  # so its host carries the name, and this is the signature placeholder rather than the composer's
  # "Write your message": the bundle has parsed and been told what surface it is.
  atspi wait --name "Write your signature" --showing --timeout 30
  capture signature-editor
  ui_press text=Save role=button window=Settings
  # The row is the core's answer, not the editor's: the library is re-read from the snapshot after
  # the create, so seeing it proves the whole round trip rather than a widget left on screen.
  ui_wait role=list-item text="New signature" window=Settings
  capture signature-created

  # AT-SPI on purpose: GTK's accessibility bridge crashed within one to three repetitions while the
  # signature editor was a separate toplevel (docs/signatures.md). Twelve edit and save cycles
  # with AT-SPI walking the tree and invoking the controls are the regression test for it.
  for _ in {1..12}; do
    atspi activate --name "Edit" --within "New signature" --within-role "list item" --timeout 20
    atspi wait --name "Write your signature" --showing --timeout 30
    atspi activate --name "Save" --within "Settings" --within-role frame --timeout 20
    atspi wait --name "Write your signature" --absent --timeout 20
  done
}

# The cross-account merge; the one contacts rule a single-account boot cannot show. `stalwart-multi`
# connects alice and bob, and the harness files the same card (`shared-*.vcf`) in both books, so one
# person with one canonical address arrives from two accounts.
#
# The assertion is the disclosure, not the dedup: a list showing "Iris Jansen" once is also what a
# client that silently dropped the second copy would show, so the row has to say it is a merge
# (docs/contacts.md §2). The detail's "Also in", which names the two accounts, stays on its widget
# test.
leg_contacts_merge() {
  MAILCAL_DEV_ACCOUNT=stalwart-multi reopen_app_on "" contacts-merge
  ui_press text=Contacts role=toggle-button
  ui_wait role=list-item text="$MERGED_CONTACT" enabled=true timeoutMs=60000
  ui_wait text="In 2 accounts" timeoutMs=30000
  open_settings Calendar
  ui_wait role=list-item text=alice@test.local window=Settings timeoutMs=60000
  ui_wait role=list-item text=bob@test.local window=Settings timeoutMs=60000
  close_settings
  capture contacts-merged
}
