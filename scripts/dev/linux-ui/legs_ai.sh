#!/usr/bin/env bash
# The Linux UI suite's writing assistant leg (docs/ai.md), on the showcase with MAILCAL_OFFER_AI,
# whose core stages two styles, an own endpoint on this device and a canned draft (docs/debugging.md).
# Sourced by test-linux-ui.sh.

# Settings → Writing style's rows, a style's detail, a new style's reveal page by page, the own
# endpoint's form under Advanced, learning from the first account until it fails with its remedy,
# and Draft a reply in a reply composer through to the card and the
# question Send asks while its checklist has open items. Nothing leaves the machine: the staged
# endpoint answers inside the core.
leg_writing_style() {
  stop_app
  MAILCAL_SHOWCASE=1 MAILCAL_OFFER_AI=1 MAILCAL_SHOWCASE_SCREEN=list \
    launch_app "" "$ARTIFACT_DIR/writing-style"
  app_ready
  open_settings "Writing style"
  ui_wait text=Work window=Settings timeoutMs=60000
  ui_wait text="Newly learned" window=Settings
  ui_wait text="Learn my writing style" role=button window=Settings
  capture writing-style-page

  # "Work"'s reveal was shown, so its row opens the detail.
  # The library's row, ahead of the account's picker that shows the same name.
  ui_press text=Work window=Settings index=0
  ui_wait text="Forget this style" role=button window=Settings
  ui_wait text="Rename" role=button window=Settings
  capture writing-style-detail
  ui_press text=Back role=button window=Settings
  ui_gone text="Forget this style" window=Settings

  # "Newly learned"'s was not, so its row opens the reveal, page by page.
  ui_press text="Newly learned" window=Settings
  ui_wait text="What we noticed about how you write" window=Settings
  capture writing-style-reveal-read
  local page
  for page in "A typical reply" "Greetings and sign-offs" "Tone and approach" "Phrases you use" \
    "Name and notes"; do
    ui_press text=Next role=button window=Settings
    ui_wait text="$page" window=Settings
  done
  capture writing-style-reveal-name
  # Save leaves the reveal, which then counts as shown: the row opens the detail from here on.
  ui_press text=Save role=button window=Settings
  ui_gone text="Name and notes" window=Settings
  ui_press text="Newly learned" window=Settings
  ui_wait text="Forget this style" role=button window=Settings
  ui_press text=Back role=button window=Settings

  # The staged endpoint, drawn back into the form; its key field says none is kept.
  ui_press text=Advanced role=toggle-button window=Settings
  ui_wait text="Own AI endpoint" window=Settings
  ui_wait type=AdwEntryRow text="http://localhost:11434/v1" window=Settings
  ui_wait text="Leave empty if the server needs none." window=Settings
  capture writing-style-endpoint

  # Learning, from the first account, whose staged sent mail reads as Ready and whose run cannot
  # reach the staged endpoint: the flow ends on Failed with its one remedy.
  ui_press text="Writing style" role=toggle-button window=Settings
  ui_press text="Learn my writing style" role=button window=Settings
  ui_wait text="Learn from which account?" window=Settings
  ui_press text=eva.jansen@example.com window=Settings
  ui_press text=Next role=button window=Settings
  ui_wait text="Which sent mail?" window=Settings
  # The account's page stays under the range's in the stack; its Next is on screen until the
  # navigation has slid it away.
  ui_gone text="Learn from which account?" window=Settings
  ui_press text=Next role=button window=Settings
  ui_wait text="What is sent, and where" window=Settings timeoutMs=30000
  capture writing-style-review
  ui_press text=Learn role=button window=Settings
  ui_wait text="No style was learned" window=Settings timeoutMs=60000
  ui_wait text="The AI service could not be reached." window=Settings
  ui_wait text="Try again" role=button window=Settings
  capture writing-style-failed
  # Closing the window closes the flow, as Cancel would.
  close_settings

  stop_app
  MAILCAL_SHOWCASE=1 MAILCAL_OFFER_AI=1 MAILCAL_SHOWCASE_SCREEN=reply \
    launch_app "" "$ARTIFACT_DIR/draft-a-reply"
  app_ready
  local has
  ui_object has text="Draft a reply"
  ui_press type=GtkMenuButton has="$has" timeoutMs=60000
  ui_wait text=Draft role=button
  ui_wait text="Thanks, I will get back to you" role=button
  capture draft-a-reply-input
  ui_press text=Draft role=button
  # Asked first while the text above the signature holds anything, or before the editor reported.
  if UI_QUIET=1 ui_wait text="Replace what you have written?" timeoutMs=3000; then
    ui_press text=Replace role=button
  fi
  ui_wait text="In short" timeoutMs=30000
  ui_wait text="Before you send"
  ui_wait text="Fill in [delivery date]"
  ui_wait text="Attach the signed quote"
  ui_wait text="Book the delivery van"
  capture draft-a-reply-card
  ui_press text=Send role=button
  ui_wait text="Send with open items?"
  capture draft-a-reply-send-question
  ui_press text="Keep editing" role=button
  ui_gone text="Send with open items?"
  # Asked once in a composition: Send now sends.
  ui_press text=Send role=button
  ui_gone text="Send with open items?" timeoutMs=3000
  ui_gone text="In short" timeoutMs=30000
}
