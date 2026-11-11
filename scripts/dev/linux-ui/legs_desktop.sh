#!/usr/bin/env bash
# The Linux UI suite's desktop legs: what the desktop hands the app (a mail link, a notification
# click) and the showcase's notices and questions, each on a launch of its own. Sourced by
# test-linux-ui.sh.

# The three recipients of MAILTO_URI, each a pill a person can see and take off.
mailto_pills_shown() { # <timeout-ms>
  ui_wait text="ada@example.test: Remove recipient" role=button timeoutMs="$1"
  ui_wait text="copy@example.test: Remove recipient" role=button
  ui_wait text="audit@example.test: Remove recipient" role=button
}

# A warm desktop activation has to dismiss Settings, here with the signature editor open in it, and
# route through the already-running GApplication instance. Bcc is deliberately present: a hidden
# seeded recipient is a security failure, so all three address pills must be visible and removable.
leg_mailto_warm() {
  ui_wait type=GtkWindow text=Settings
  ui_within press role=list-item text="New signature" window=Settings -- text=Edit role=button
  forward_mailto "$MAILTO_URI"
  ui_gone type=GtkWindow text=Settings
  mailto_pills_shown 30000
  grep -Fq "$MAILTO_URI" "$APP_LOG" 2>/dev/null && die "mail link content reached the diagnostic log"
  capture mailto-composer
  ui_press text=Discard role=button
  ui_gone id=composer-editor
}

# Cold activation exercises the other half of the desktop contract: the first process receives the
# raw URI before its window is active and still brings an exact, pre-filled draft forward.
leg_mailto_cold() {
  stop_app
  launch_app "" "$ARTIFACT_DIR/mailto-cold" "$MAILTO_URI"
  app_ready
  mailto_pills_shown 60000
  capture mailto-cold
  ui_press text=Discard role=button
  ui_gone id=composer-editor
}

# Click the notification posted for <subject>, as the desktop does: through the app's bus name.
click_notification() { # <subject>
  "$PYTHON" "$REPO_ROOT/scripts/dev/linux_notification_portal.py" click \
    "$PORTAL_CAPTURE" "$1" "$MAILCAL_APP_ID" ||
    die "clicking the notification for '$1' did not reach the app"
}

# The click opened <subject>: the reading pane's heading names it beside the list row that does.
# Asked from a screen showing neither, so a click that did nothing, or that brought the mailbox back
# without opening the message, finds one match or none.
opened_from_notification() { # <subject>
  ui_wait text="$1" role=label index=1 timeoutMs=60000
}

# The same click with the app not running: the bus starts it from a service file, as it starts the
# Flatpak from the one that ships (docs/background-sync.md).
click_notification_cold() { # <subject>
  stop_app
  local services="$XDG_DATA_HOME/dbus-1/services"
  mkdir -p "$services"
  printf '[D-BUS Service]\nName=%s\nExec=%s --activated-app --artifacts %s\n' \
    "$MAILCAL_APP_ID" "$SELF" "$ARTIFACT_DIR" >"$services/$MAILCAL_APP_ID.service"
  # The bus reads a service directory when it starts, and this one did not exist yet. The
  # accessibility bus was started inside the session, so the bus's own copy of the environment,
  # which is what it starts a service with, has never heard of it.
  gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
    --method org.freedesktop.DBus.ReloadConfig >/dev/null
  gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
    --method org.freedesktop.DBus.UpdateActivationEnvironment \
    "{'AT_SPI_BUS_ADDRESS': '$AT_SPI_BUS_ADDRESS'}" >/dev/null
  click_notification "$1"
  APP_PID="$(cat "$ARTIFACT_DIR/notification-cold.pid")"
  app_ready
}

# A click on a new-mail notification, delivered as GNOME Shell delivers it: the notification's own
# `app.` action, called on the app's bus name. Asked over the calendar, because a click takes the
# app to mail before it opens anything, and from there neither the row nor the heading is on
# screen. Warm first, then with the app quit, which is the launch the bus has to make.
leg_notification_click() {
  local clicked_warm="Linux notification click $RANDOM-$RANDOM"
  notified_for "$clicked_warm" || die "the notification to click never reached the desktop portal"
  show_calendar
  click_notification "$clicked_warm"
  opened_from_notification "$clicked_warm"
  capture notification-click

  local clicked_cold="Linux notification cold click $RANDOM-$RANDOM"
  notified_for "$clicked_cold" || die "the notification to click never reached the desktop portal"
  click_notification_cold "$clicked_cold"
  opened_from_notification "$clicked_cold"
  capture notification-click-cold
}

# The question a calendar paused for a server on another site asks (docs/account-autodetect.md
# rule 14), on the showcase: MAILCAL_SHOWCASE_SERVER_CHANGE has the core pause its first account's
# calendar for a move from example.com to example.net, which no harness fixture can stage. Neither
# key a person might press to get past it allows anything: Escape is the close response and Return
# the default, and both are "Not now", so those two are real keystrokes. A showcase account has no
# servers to change, so the core refuses the allow, and the question says so itself and stays open
# to be asked again, which is what proves Allow reached it.
leg_server_change() {
  stop_app
  MAILCAL_SHOWCASE=1 MAILCAL_SHOWCASE_SERVER_CHANGE=1 MAILCAL_SHOWCASE_SCREEN=calendar \
    launch_app "" "$ARTIFACT_DIR/server-change"
  app_ready
  local paused="Calendar paused: waiting for you to confirm a new server."
  local asked="Your calendar server has changed"
  local refused="The new server couldn’t be reached, so nothing was changed. Please try again later."
  ui_wait textLike="$paused" timeoutMs=60000
  local key
  for key in Escape Return; do
    ui_press text=Review role=button
    ui_wait text="$asked"
    ui_wait textLike="Before: example.com"
    ui_wait textLike="Now: example.net"
    [[ "$key" == Return ]] || capture server-change-asked
    linux_session_type -k "$key"
    ui_gone text="$asked"
    ui_wait textLike="$paused"
    ui_wait text=Review role=button
  done

  ui_press text=Review role=button
  ui_wait text="$asked"
  ui_press text="Allow new server" role=button
  ui_wait text="$refused" timeoutMs=30000
  ui_wait text="$asked"
  ui_wait text="Allow new server" role=button enabled=true
  capture server-change-refused
  linux_session_type -k Escape
  ui_gone text="$asked"
  ui_wait textLike="$paused"
}

# The core's notices (docs/notices.md), on the showcase: MAILCAL_SHOWCASE_REAUTH has the core ask
# its first account to sign in again for mail and for its calendar, which no harness fixture can
# stage. Mail's notice holds the mail screen's banner, the calendar's the calendar screen's, and the
# account both are about reads "2 issues" beneath its address. An AdwBanner has one button, so Sign
# in again on it is also the proof that it offers no Close. The twins are
# clients/apple/UITests/NoticeTests.swift and
# clients/windows/Mailcal.UiTests/Suites/ReauthNoticeTests.cs.
leg_reauth_notices() {
  stop_app
  MAILCAL_SHOWCASE=1 MAILCAL_SHOWCASE_REAUTH=1 MAILCAL_SHOWCASE_SCREEN=list \
    launch_app "" "$ARTIFACT_DIR/reauth-notices"
  app_ready
  local mail_reauth="Sending and managing mail needs your permission for"
  local calendar_reauth="Calendar access needs your permission for"
  ui_wait textLike="$mail_reauth" timeoutMs=60000
  ui_wait text="Sign in again" role=button
  ui_wait text="2 issues"
  ui_press text="1 more issue"
  ui_wait text=Issues
  # AT-SPI: the Issues list's button joins its answer and the notice's sentence into one spoken
  # name, which exists only for a screen reader.
  atspi wait --name-substring "Sign in again: $calendar_reauth" \
    --role "push button" --showing --timeout 20
  capture reauth-notices-issues
  linux_session_type -k Escape
  ui_gone text=Issues
  capture reauth-notices-mail
  show_calendar_tab
  ui_wait textLike="$calendar_reauth" timeoutMs=30000
  ui_wait text="Sign in again" role=button
  capture reauth-notices-calendar
}

# The showcase has no New Event to wait for while its calendar asks to sign in, so this only moves.
show_calendar_tab() {
  ui_press text=Calendar role=toggle-button
}

# The core's questions (docs/notices.md rule 8), on the showcase: MAILCAL_SHOWCASE_QUESTIONS has the
# core raise all three, so everything from the question to the button is real. The Issues list holds
# every notice in force, so the copy's and the reply's are answered there, whichever of them the
# banner holds; the time zone question ranks last, so once the other two are answered it holds the
# banner, and its answers are the strip's. Each lesser answer takes its own notice away. The twins
# are clients/apple/UITests/QuestionNoticeTests.swift and Android's QuestionNoticesTest.
leg_question_notices() {
  stop_app
  MAILCAL_SHOWCASE=1 MAILCAL_SHOWCASE_QUESTIONS=1 MAILCAL_SHOWCASE_SCREEN=list \
    launch_app "" "$ARTIFACT_DIR/question-notices"
  app_ready
  ui_press text="2 more issues" timeoutMs=60000
  ui_wait text=Issues
  # AT-SPI: each answer in the Issues list is spoken as "<answer>: <question>", so a screen reader
  # knows which question it answers; the visible button says the answer alone.
  local answer
  for answer in "Save to Sent" "Not now" "Send the email" "Don’t send" "Update" "Keep current"; do
    atspi wait --name-substring "$answer: " --role "push button" --showing --timeout 20
  done
  capture question-notices-issues
  # Each answer is pressed inside the Issues list, whose button names how many there are.
  ui_one text="2 more issues"
  ui_press under="$UI_H" text="Not now" role=button
  ui_press text="1 more issue"
  ui_one text="1 more issue"
  local issues="$UI_H"
  ui_wait text=Issues
  ui_gone under="$issues" text="Save to Sent" role=button
  ui_press under="$issues" text="Don’t send" role=button
  ui_gone text="1 more issue"
  local zone_asked="Update the display time zone?"
  ui_wait textLike="$zone_asked"
  ui_wait text=Update role=button
  capture question-notices-time-zone
  ui_press text="Keep current" role=button
  ui_gone textLike="$zone_asked"
}
