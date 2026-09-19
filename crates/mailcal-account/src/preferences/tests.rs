//! Round-trip, default-fallback, and forward-compatibility tests for the preferences
//! file. Split from `preferences.rs` to keep it under the 500-line limit.

use std::fs;

use super::*;

#[test]
fn missing_file_loads_defaults() {
    let prefs = load_preferences("/nonexistent/dir/preferences.toml");
    assert_eq!(prefs, Preferences::default());
    assert_eq!(prefs.display_timezone, None);
}

#[test]
fn save_then_load_round_trips_the_preferences() {
    let dir = std::env::temp_dir().join("mailcal-prefs-test-roundtrip");
    let _ = fs::remove_dir_all(&dir);
    let path = dir.join("nested/preferences.toml");
    let prefs = Preferences {
        display_timezone: Some("Europe/Amsterdam".to_owned()),
        default_send_account: Some("me@imap.example.com".to_owned()),
        message_grouping: MessageGrouping::Flat,
        quote_style: QuoteStyle::LineAndHeader,
        quote_style_per_message: true,
        swipe_left: SwipeAction::Archive,
        swipe_right: SwipeAction::Star,
        week_start: WeekStart::Sunday,
        time_format: TimeFormat::TwelveHour,
        appearance: Appearance::Dark,
        calendar_visible_hours: 8,
        calendar_layout: CalendarLayout::ThreeDay,
        calendars: BTreeMap::from([(
            "me@imap.example.com".to_owned(),
            BTreeMap::from([(
                "work".to_owned(),
                CalendarPrefs {
                    visible: false,
                    color: Some("#3f8f55".to_owned()),
                },
            )]),
        )]),
        default_calendar: Some(DefaultCalendar {
            account: "me@imap.example.com".to_owned(),
            calendar: "work".to_owned(),
        }),
        accounts: BTreeMap::from([(
            "me@imap.example.com".to_owned(),
            AccountSyncSettings {
                strategy: SyncStrategy::Push,
                push_folders: vec!["INBOX".to_owned(), "Archive".to_owned()],
                poll_interval_mins: 60,
                sync_depth: Some(SyncDepth::Months(6)),
                message_size_limit: None,
            },
        )]),
        notify_marks: BTreeMap::from([(
            "me@imap.example.com".to_owned(),
            "2026-06-01T09:30:00Z".to_owned(),
        )]),
        analytics_consent: Some(true),
        default_mail_app_offer: Some(false),
        analytics_install_id: Some("k7VqZ3mQ0pR1sT2uV3wX4g".to_owned()),
        analytics_notice_version: Some(1),
        analytics_consented_at: Some("2026-07-11T10:00:00Z".to_owned()),
        signature_assignments: BTreeMap::from([(
            "me@imap.example.com".to_owned(),
            AccountSignatureAssignment {
                new_message: SignatureId::new("kK3-x_9"),
                reply_forward: None,
            },
        )]),
        invitation_reply_fallback: BTreeMap::from([(
            "me@imap.example.com".to_owned(),
            ReplyFallback::Always,
        )]),
        account_sender_names: BTreeMap::new(),
        account_aliases: BTreeMap::from([(
            "me@imap.example.com".to_owned(),
            vec!["info@example.com".to_owned()],
        )]),
        mcp_enabled: true,
        mcp_notice_version: Some(1),
        mcp_accounts: BTreeSet::from(["me@imap.example.com".to_owned()]),
        mcp_allow_direct_send: true,
        mcp_require_known_recipient: false,
        collapsed_accounts: BTreeSet::from(["me@imap.example.com".to_owned()]),
        unified_collapsed: true,
        collapsed_folders: BTreeMap::from([(
            "me@imap.example.com".to_owned(),
            BTreeSet::from(["Archive".to_owned()]),
        )]),
    };
    // Saving creates the nested parent dirs and the load reads the values back.
    save_preferences(&path, &prefs).unwrap();
    assert_eq!(load_preferences(&path), prefs);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_older_preferences_file_has_chosen_no_default_calendar() {
    // A file written before the setting existed must read as "nobody chose", which is what falls
    // back to the first writable calendar: not as a choice naming nothing.
    let prefs: Preferences = toml::from_str("display_timezone = \"Europe/Amsterdam\"").unwrap();
    assert!(prefs.default_calendar.is_none());
}

#[test]
fn default_sync_depth_is_three_months() {
    assert_eq!(SyncDepth::default(), SyncDepth::Months(3));
}

#[test]
fn an_older_preferences_file_without_notify_marks_defaults_to_empty() {
    // A file written before background-sync notifications existed still loads, with the
    // marks defaulting to an empty map rather than erroring.
    let prefs: Preferences = toml::from_str("display_timezone = \"Europe/Amsterdam\"").unwrap();
    assert!(prefs.notify_marks.is_empty());
}

#[test]
fn an_older_preferences_file_opens_every_folder_tree() {
    // A file written before the folder pane persisted anything must not read as "every
    // account collapsed", which is what an expanded-accounts set would have done.
    let prefs: Preferences = toml::from_str("display_timezone = \"Europe/Amsterdam\"").unwrap();
    assert!(prefs.collapsed_accounts.is_empty());
    assert!(prefs.account_expanded("me@imap.example.com"));
    // And the All Accounts group with them, for the same reason.
    assert!(prefs.unified_expanded());
    // And every folder inside every account: the launch this ships in must not hide a folder
    // that was on screen the launch before.
    assert!(prefs.collapsed_folders.is_empty());
    assert!(prefs.folder_expanded("me@imap.example.com", "Archive"));
}

#[test]
fn an_older_preferences_file_still_follows_the_host_appearance() {
    // The default is the one that is *invisible* if it regresses: a file written before this
    // setting existed must keep following the host's light/dark choice, not pin itself to light.
    let older: Preferences = toml::from_str("display_timezone = \"Europe/Amsterdam\"").unwrap();
    assert_eq!(older.appearance, Appearance::System);
    let stored: Preferences = toml::from_str("appearance = \"dark\"").unwrap();
    assert_eq!(stored.appearance, Appearance::Dark);
}

#[test]
fn default_quote_style_is_indented() {
    assert_eq!(QuoteStyle::default(), QuoteStyle::Indented);
    assert_eq!(Preferences::default().quote_style, QuoteStyle::Indented);
}

#[test]
fn quote_style_parses_from_snake_case_and_defaults_when_absent() {
    let prefs: Preferences = toml::from_str("quote_style = \"line_and_header\"").unwrap();
    assert_eq!(prefs.quote_style, QuoteStyle::LineAndHeader);
    // A file written before the setting existed still loads, defaulting to Indented.
    let older: Preferences = toml::from_str("display_timezone = \"Europe/Amsterdam\"").unwrap();
    assert_eq!(older.quote_style, QuoteStyle::Indented);
}

#[test]
fn a_preferences_file_written_under_the_old_style_names_keeps_the_users_choice() {
    // The setting shipped as `gmail` / `outlook` before the styles were renamed for what
    // they are. An existing file must keep meaning what the user chose, not silently reset
    // to the default: so both old tokens still deserialize.
    let old_default: Preferences = toml::from_str("quote_style = \"gmail\"").unwrap();
    assert_eq!(old_default.quote_style, QuoteStyle::Indented);
    let old_other: Preferences = toml::from_str("quote_style = \"outlook\"").unwrap();
    assert_eq!(old_other.quote_style, QuoteStyle::LineAndHeader);
    // Writing back re-serializes under the current name; the alias is read-only.
    let body = toml::to_string(&old_other).unwrap();
    assert!(body.contains("quote_style = \"line_and_header\""), "{body}");
}

#[test]
fn the_per_message_quote_override_is_off_by_default_and_round_trips() {
    assert!(!Preferences::default().quote_style_per_message);
    let prefs: Preferences = toml::from_str("quote_style_per_message = true").unwrap();
    assert!(prefs.quote_style_per_message);
    // A file written before the toggle existed loads with the picker hidden: the composer
    // just uses the app default, which is the behaviour we want for everyone who never asked
    // for the advanced control.
    let older: Preferences = toml::from_str("display_timezone = \"Europe/Amsterdam\"").unwrap();
    assert!(!older.quote_style_per_message);
}

#[test]
fn both_swipe_directions_default_to_delete() {
    assert_eq!(SwipeAction::default(), SwipeAction::Delete);
    assert_eq!(Preferences::default().swipe_left, SwipeAction::Delete);
    assert_eq!(Preferences::default().swipe_right, SwipeAction::Delete);
}

#[test]
fn swipe_actions_parse_from_snake_case_and_default_when_absent() {
    let prefs: Preferences =
        toml::from_str("swipe_left = \"archive\"\nswipe_right = \"star\"").unwrap();
    assert_eq!(prefs.swipe_left, SwipeAction::Archive);
    assert_eq!(prefs.swipe_right, SwipeAction::Star);
    // A file written before the setting existed still loads, keeping the old
    // both-directions-delete behaviour rather than erroring.
    let older: Preferences = toml::from_str("display_timezone = \"Europe/Amsterdam\"").unwrap();
    assert_eq!(older.swipe_left, SwipeAction::Delete);
    assert_eq!(older.swipe_right, SwipeAction::Delete);
}

#[test]
fn default_send_account_round_trips_and_defaults_to_none() {
    let prefs: Preferences = toml::from_str("default_send_account = \"acct-2\"").unwrap();
    assert_eq!(prefs.default_send_account.as_deref(), Some("acct-2"));
    // Absent (an older file, or a user who never chose one) means "derive it".
    let older: Preferences = toml::from_str("display_timezone = \"Europe/Amsterdam\"").unwrap();
    assert_eq!(older.default_send_account, None);
}

#[test]
fn analytics_defaults_to_unasked_with_no_install_id() {
    // The whole gate: absent consent is "not asked", which the core treats as OFF. An
    // older preferences file (written before analytics existed) must land here too;
    // upgrading the app never opts anyone in.
    let default = Preferences::default();
    assert_eq!(default.analytics_consent, None);
    assert_eq!(default.analytics_install_id, None);
    assert_eq!(default.analytics_notice_version, None);
    assert_eq!(default.analytics_consented_at, None);

    let older: Preferences = toml::from_str("display_timezone = \"Europe/Amsterdam\"").unwrap();
    assert_eq!(older.analytics_consent, None);
    assert_eq!(older.analytics_install_id, None);
}

#[test]
fn the_default_mail_app_offer_starts_unasked_and_tells_its_answers_apart() {
    // The same tri-state the analytics consent above keeps, and for the same reason: `None` is
    // the only value that lets the offer be put, so an older preferences file (written before
    // the offer existed) must read back as never-offered rather than as answered.
    assert_eq!(Preferences::default().default_mail_app_offer, None);
    let older: Preferences = toml::from_str("display_timezone = \"Europe/Amsterdam\"").unwrap();
    assert_eq!(older.default_mail_app_offer, None);

    let declined: Preferences = toml::from_str("default_mail_app_offer = false").unwrap();
    assert_eq!(declined.default_mail_app_offer, Some(false));
    let accepted: Preferences = toml::from_str("default_mail_app_offer = true").unwrap();
    assert_eq!(accepted.default_mail_app_offer, Some(true));
}

#[test]
fn a_declined_consent_is_distinguishable_from_an_unasked_one() {
    // `Some(false)` (asked, declined) must not read back as `None` (never asked); the
    // difference is whether the app is allowed to put the question up again.
    let declined: Preferences = toml::from_str("analytics_consent = false").unwrap();
    assert_eq!(declined.analytics_consent, Some(false));
    assert_eq!(declined.analytics_install_id, None);

    let unasked: Preferences = toml::from_str("").unwrap();
    assert_eq!(unasked.analytics_consent, None);
}

#[test]
fn default_message_grouping_is_threaded() {
    assert_eq!(MessageGrouping::default(), MessageGrouping::Threaded);
    assert_eq!(
        Preferences::default().message_grouping,
        MessageGrouping::Threaded
    );
}

#[test]
fn message_grouping_parses_from_snake_case_and_defaults_when_absent() {
    let prefs: Preferences = toml::from_str("message_grouping = \"flat\"").unwrap();
    assert_eq!(prefs.message_grouping, MessageGrouping::Flat);
    // A file written before the setting existed still loads, defaulting to Threaded (the
    // product default) rather than erroring.
    let older: Preferences = toml::from_str("display_timezone = \"Europe/Amsterdam\"").unwrap();
    assert_eq!(older.message_grouping, MessageGrouping::Threaded);
}

#[test]
fn account_sync_depth_round_trips_and_an_older_entry_defaults_to_none() {
    // An explicit per-account override serializes and reads back.
    let prefs: Preferences =
        toml::from_str("[accounts.\"me@imap.example.com\"]\nstrategy = \"poll\"\nsync_depth = 6\n")
            .unwrap();
    assert_eq!(
        prefs.accounts["me@imap.example.com"].sync_depth,
        Some(SyncDepth::Months(6))
    );
    // An account entry written before per-account depth existed loads with `None`, so it
    // inherits the product default rather than erroring.
    let older: Preferences =
        toml::from_str("[accounts.\"me@imap.example.com\"]\nstrategy = \"poll\"\n").unwrap();
    assert_eq!(older.accounts["me@imap.example.com"].sync_depth, None);
}

#[test]
fn effective_sync_depth_prefers_the_account_override_else_product_default() {
    let mut prefs = Preferences::default();
    // An account with no entry uses the product default.
    assert_eq!(
        prefs.effective_sync_depth("me@imap.example.com"),
        SyncDepth::Months(3)
    );
    // An entry present but without an override still uses the product default.
    prefs.accounts.insert(
        "me@imap.example.com".to_owned(),
        AccountSyncSettings {
            strategy: SyncStrategy::Poll,
            push_folders: Vec::new(),
            poll_interval_mins: DEFAULT_POLL_INTERVAL,
            sync_depth: None,
            message_size_limit: None,
        },
    );
    assert_eq!(
        prefs.effective_sync_depth("me@imap.example.com"),
        SyncDepth::Months(3)
    );
    // An explicit override wins over the default.
    prefs
        .accounts
        .get_mut("me@imap.example.com")
        .unwrap()
        .sync_depth = Some(SyncDepth::AllTime);
    assert_eq!(
        prefs.effective_sync_depth("me@imap.example.com"),
        SyncDepth::AllTime
    );
}

#[test]
fn an_older_preferences_file_without_accounts_defaults_to_empty() {
    // A file written before per-account sync settings existed still loads; the map
    // defaults to empty rather than erroring, so every account uses the product default.
    let prefs: Preferences = toml::from_str("display_timezone = \"Europe/Amsterdam\"").unwrap();
    assert!(prefs.accounts.is_empty());
}

#[test]
fn corrupt_file_falls_back_to_defaults() {
    let dir = std::env::temp_dir().join("mailcal-prefs-test-corrupt");
    let _ = fs::remove_dir_all(&dir);
    let path = dir.join("preferences.toml");
    fs::create_dir_all(&dir).unwrap();
    fs::write(&path, "this is = not valid = toml =").unwrap();
    assert_eq!(load_preferences(&path), Preferences::default());
    let _ = fs::remove_dir_all(&dir);
}
