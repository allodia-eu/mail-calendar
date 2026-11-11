#!/usr/bin/env bash
# The Linux UI suite's first legs: the screens a launch raises, the mail window's chrome, Settings'
# Privacy and Diagnostics, the MCP server and new-mail notifications. Sourced by test-linux-ui.sh.

leg_welcome() {
  ui_wait type=GtkWindow textLike="Welcome to $MAILCAL_APP_NAME" timeoutMs=60000
  ui_switch true text="Share usage statistics" role=switch
  ui_press text="See exactly what we send" role=button
  ui_wait type=GtkWindow text="See exactly what we send"
  ui_press text=Close role=button window="See exactly what we send"
  ui_press text="Get started" role=button
  ui_gone type=GtkWindow textLike="Welcome to $MAILCAL_APP_NAME"

  eventually grep -q '^analytics_consent = true$' "$CORE_PREFERENCES" ||
    die "analytics consent was not persisted"
  grep -q '^analytics_install_id = ' "$CORE_PREFERENCES" ||
    die "analytics consent minted no install id"
}

# The device zone this run fakes is a question the core asks as a notice (docs/notices.md rule 8),
# answered in the banner's strip.
leg_zone_question() {
  local zone_question="Your device is now in Pacific/Auckland."
  ui_wait textLike="$zone_question"
  ui_press text="Keep current" role=button
  ui_gone textLike="$zone_question"
  MAILCAL_FAKE_DEVICE_TIMEZONE=""
}

# The mail command bar at two widths (docs/command-bar.md, rule 4). Wide, every item stands on
# the bar and Report waits behind ⋯; narrowed by the compositor, the items that no longer fit
# leave the bar from the end, Sync first, and are found behind ⋯ instead, under their own names.
# A control that has moved leaves the accessibility tree with the bar, so `--absent` is the
# question of whether it is still drawn there.
#
# AT-SPI throughout: the bar is a toolbar to a screen reader, and each item a push button inside
# it, which is the role this asks about as much as the place.
command_bar_narrows() {
  local bar=(--within-role "tool bar")
  for item in "Delete" "Archive" "Move" "Flag" "Mark as read" "Sync"; do
    atspi wait --name "$item" --role "push button" "${bar[@]}" --showing --timeout 20
  done
  capture command-bar-wide

  SWAYSOCK="$LINUX_SESSION_SWAYSOCK" swaymsg output HEADLESS-1 resolution 700x900 >/dev/null
  atspi wait --name "Sync" --role "push button" "${bar[@]}" --absent --timeout 20
  atspi wait --name "Delete" --role "push button" "${bar[@]}" --showing --timeout 20
  atspi activate --name "More actions" --role "toggle button" "${bar[@]}" --timeout 20
  atspi wait --name "Sync" --role "push button" "${bar[@]}" --showing --timeout 20
  atspi wait --name "Mark as Spam" --role "push button" "${bar[@]}" --timeout 20
  capture command-bar-narrow
  # Sync from the menu runs as Sync from the bar, and picking it closes the menu.
  atspi activate --name "Sync" --role "push button" "${bar[@]}" --timeout 20
  atspi wait --name "Mark as Spam" --role "push button" "${bar[@]}" --absent --timeout 20

  SWAYSOCK="$LINUX_SESSION_SWAYSOCK" swaymsg output HEADLESS-1 resolution 1440x900 >/dev/null
  ui_wait text=Sync role=button
}

leg_mailbox() {
  ui_wait id=message-list timeoutMs=60000
  ui_wait text="New Mail" role=button enabled=true
  ui_wait type=GtkSearchEntry enabled=true
  ui_wait text=Sync role=button enabled=true
  command_bar_narrows

  # The All Accounts group's chevron, which folds the group and keeps that across a restart.
  ui_wait role=list-item text=Inbox index=1 enabled=true
  ui_within press role=list-item text="All Accounts" -- text="Hide folders" role=button
  eventually grep -q '^unified_collapsed = true$' "$CORE_PREFERENCES" ||
    die "the All Accounts group did not persist its shut state"
  ui_within press role=list-item text="All Accounts" -- text="Show folders" role=button
  eventually grep -q '^unified_collapsed = false$' "$CORE_PREFERENCES" ||
    die "the All Accounts group did not persist its open state"
  ui_wait role=list-item text=Inbox index=1 enabled=true

  # The switch to Archive is timed until the Inbox's top row leaves, read off the screen rather than
  # named here: which message that is depends on what earlier runs delivered, and a row the list
  # never built is absent before the switch starts.
  ui_one id=message-list
  local list="$UI_H" inbox_top started archive_ms inbox_ms
  ui_wait under="$list" role=list-item index=0 timeoutMs=30000
  inbox_top="$(ui_first_text)" || die "the Inbox showed no message row to switch away from"
  started="$EPOCHREALTIME"
  ui_press role=list-item text=Archive index=0
  ui_gone role=list-item text="$inbox_top" timeoutMs=30000
  archive_ms="$(elapsed_ms "$started")"
  started="$EPOCHREALTIME"
  ui_press role=list-item text=Inbox index=0
  ui_wait under="$list" role=list-item timeoutMs=30000
  inbox_ms="$(elapsed_ms "$started")"
  printf 'archive_ms=%s\nall_inboxes_ms=%s\n' "$archive_ms" "$inbox_ms" |
    tee "$ARTIFACT_DIR/folder-switch-timing.txt"
  ui_wait text="Downloading 1,200 of 3,387…"
  capture essential-feedback

  local calendar_refreshes=0
  for _ in {1..100}; do
    calendar_refreshes="$(grep -c 'refresh_calendar:' "$APP_LOG" 2>/dev/null || true)"
    (( calendar_refreshes >= 2 )) && break
    sleep 0.1
  done
  (( calendar_refreshes >= 2 )) || die "foreground calendar timer did not refresh twice"
}

# Settings is one reachable taxonomy, not a collection of launch-only screens. Exercise the privacy
# lifecycle and the complete diagnostic-log path before returning to mail.
leg_settings() {
  open_settings Privacy
  ui_wait text="Usage statistics"
  ui_switch false text="Share usage statistics" role=switch
  eventually grep -q '^analytics_consent = false$' "$CORE_PREFERENCES" ||
    die "analytics withdrawal was not persisted"
  grep -q '^analytics_install_id = ' "$CORE_PREFERENCES" &&
    die "analytics withdrawal kept the install id"

  ui_press text=Diagnostics role=toggle-button window=Settings
  ui_wait text="Log size"
  ui_press text="View log" role=button
  ui_wait type=GtkWindow text="Diagnostic log"
  ui_wait text="Jump to end" role=button
  ui_press text=Close role=button window="Diagnostic log"
  ui_press text="Copy path" role=button
  ui_press text="Export log…" role=button window=Settings
  ui_wait type=GtkWindow text="Share diagnostic log?"
  ui_wait textLike="never contains message content" role=label
  ui_press text="Export log…" role=button window="Share diagnostic log?"
  eventually test -s "$ARTIFACT_DIR/exported-mailcal.log" || die "diagnostic log export wrote no file"
  ui_switch true text="Include more detail" role=switch
  eventually grep -q '"diagnostics_debug": true' "$HOST_PREFERENCES" ||
    die "diagnostic detail setting was not persisted"
  close_settings
}

# Grant each layer separately through Settings, then drive the shipped relay over its real Unix
# socket. Four calls in one session catch a relay that answers exactly once, while the tool listing
# proves direct send stays absent until its separate toggle is granted. The final call crosses
# back onto GTK's main thread and opens the ordinary composer, unsent.
leg_mcp() {
  open_settings Advanced
  ui_wait text="AI assistant access"
  ui_wait id=mcp-status text=Off
  ui_switch true id=mcp-enabled-toggle
  ui_wait id=mcp-status text="Running · waiting for a client to connect"
  ui_wait id=mcp-accounts-empty
  ui_switch true text=alice@test.local role=switch
  ui_gone id=mcp-accounts-empty
  ui_press id=mcp-copy-config
  # A confirmation that goes again after a moment: asked straight after the press, on the open
  # connection, rather than from a process started afterwards.
  ui_wait id=mcp-copy-config text=Copied
  local mcp_config="$ARTIFACT_DIR/mcp-config.json"
  ui_wait id=mcp-config-snippet
  ui_value >"$mcp_config"
  mcp_config_is_the_relay "$mcp_config"
  capture mcp-settings
  close_settings

  local mcp_endpoint="$XDG_DATA_HOME/$MAILCAL_DATA_DIR_NAME/mcp.sock"
  eventually test -S "$mcp_endpoint" || die "enabling MCP did not bind its Unix socket"
  mcp_session "$mcp_config"
  ui_wait text="mcp-recipient@example.test: Remove recipient" role=button enabled=true \
    timeoutMs=30000
  ui_wait id=composer-editor timeoutMs=30000
  capture mcp-draft
  # Nobody has written in it, so there is nothing to lose: Discard closes it without asking.
  ui_press text=Discard role=button
  ui_gone id=composer-editor
  mcp_session_end

  # MCP off, on the way past: its socket must go with it.
  open_settings Advanced
  ui_switch false id=mcp-enabled-toggle
  ui_wait id=mcp-status text=Off
  eventually test ! -e "$mcp_endpoint" || die "turning MCP off left its socket behind"
  close_settings
}

# New-mail notifications: on (the shipped default), off, on again. The middle one asserts an
# **absence**, which proves nothing by itself. A path that never fires satisfies it silently, and
# did, for as long as the portal call hung; so the silence is only believed once a *later*
# notification has provably crossed, which is what the third delivery is for.
#
# That third delivery is also the second post of the session, and the second is where this breaks:
# the process shares one portal connection, and a caller owning the runtime it was opened on takes
# it down on the way out, so the first notification lands and every later one hangs for good
# (docs/client-traps.md). Checking the quiet subject only at the end is what makes both halves
# real, because a notification cannot still be in flight once a later one has arrived.
leg_notifications() {
  local notification_on="Linux notification on $RANDOM-$RANDOM"
  local notification_off="Linux notification off $RANDOM-$RANDOM"
  NOTIFICATION_AFTER="Linux notification after $RANDOM-$RANDOM"

  notified_for "$notification_on" ||
    die "an enabled new-mail notification never reached the desktop portal"

  set_notifications false
  if notified_for "$notification_off"; then
    die "a notification crossed the portal while notifications were disabled"
  fi

  set_notifications true
  notified_for "$NOTIFICATION_AFTER" ||
    die "new-mail notifications stopped after the first one, or after being switched back on"

  if grep -Fq "$notification_off" "$PORTAL_CAPTURE"; then
    die "the disabled notification reached the portal late, once a later one had woken the path"
  fi
  capture settings-feedback
}

# Deliver <subject>, pull it in, and answer whether a notification for it reached the fake portal.
#
# The wait is the same either way round, so a "no" costs the full ten seconds rather than a guessed
# settle: the disabled half of the notification leg asserts on this answer, and a sleep short
# enough to keep the suite quick is a sleep that calls a slow notification an absent one.
notified_for() { # <subject>
  "$REPO_ROOT/scripts/dev/harness.sh" deliver --subject "$1" >/dev/null
  ui_press text=Sync role=button
  ui_wait role=list-item text="$1" timeoutMs=60000
  for _ in {1..200}; do
    grep -Fq "$1" "$PORTAL_CAPTURE" 2>/dev/null && return 0
    sleep 0.05
  done
  return 1
}

# Open Settings → Notifications, flip the switch, and wait for the host preferences file to say so.
# Reading it back matters: an activation that missed leaves the switch where it was, and every
# assertion after it would then be testing the opposite of what it names.
set_notifications() { # <true|false>
  open_settings Notifications
  ui_switch "$1" text="New-mail notifications" role=switch
  local wanted="\"notifications_enabled\": $1"
  eventually grep -Fq "$wanted" "$HOST_PREFERENCES" ||
    die "the new-mail notification switch did not persist $1"
  close_settings
}
