//! Widget-level regressions for the mailbox rows and the composer pane.

use std::collections::HashSet;

use adw::prelude::*;
use mailcal_bindings::{FlatRow, SnapshotRow, ThreadMessage, ThreadRow};

use super::{MailboxRendering, ThreadKey, display_row, flat_row};
use crate::ui::{
    AppInput,
    composer::ComposerPane,
    composer_header::tests as composer_header,
    composer_model::{ComposeKind, ComposeRequest},
    connectivity::tests as connectivity,
    contacts::pane::tests as contacts,
    destinations::tests as destinations,
    folder_pane::tests as folder_pane,
    invitation::widget_tests as invitation,
    recipients::field::tests as recipients,
    search::bar::tests as search,
    settings::signatures::tests as signatures,
};

fn fixture(subject: &str) -> FlatRow {
    fixture_from(subject, "sender")
}

fn fixture_from(subject: &str, from: &str) -> FlatRow {
    let mut avatar = crate::ui::model::blank_avatar();
    avatar.initials = "S".to_owned();
    FlatRow {
        avatar,
        account: "fixture".to_owned(),
        key: "1".to_owned(),
        subject: subject.to_owned(),
        from: from.to_owned(),
        date: "2026-07-20".to_owned(),
        unread: true,
        flagged: false,
        has_attachment: true,
        preview: String::new(),
    }
}

#[test]
fn flat_snapshot_projects_without_losing_state() {
    let display = display_row(
        &SnapshotRow::Flat {
            row: fixture("Quarterly planning"),
        },
        "Europe/Amsterdam",
    );
    assert_eq!(display.title, "Quarterly planning");
    assert_eq!(display.subtitle, "sender");
    assert_eq!(display.date, "2026-07-20");
    assert!(display.unread);
    assert!(!display.flagged);
    assert!(display.has_attachment);
    assert_eq!(display.zone, "Europe/Amsterdam");
}

#[test]
fn a_flag_change_is_part_of_the_row_rendering_key() {
    let mut row = fixture("Flagged");
    row.flagged = true;
    let display = display_row(&SnapshotRow::Flat { row }, "UTC");
    assert!(display.flagged);
}

#[test]
fn a_time_zone_change_is_part_of_the_row_rendering_key() {
    let mut snapshot = crate::ui::model::empty_mailbox();
    snapshot.rows = vec![SnapshotRow::Flat {
        row: fixture("Time zone"),
    }];

    assert_ne!(
        MailboxRendering::new(&snapshot, "UTC"),
        MailboxRendering::new(&snapshot, "Europe/Amsterdam")
    );
}

#[test]
fn a_photo_arriving_is_part_of_the_row_rendering_key() {
    let mut snapshot = crate::ui::model::empty_mailbox();
    snapshot.rows = vec![SnapshotRow::Flat {
        row: fixture("Photo"),
    }];
    let before = MailboxRendering::new(&snapshot, "UTC");
    let SnapshotRow::Flat { row } = &mut snapshot.rows[0] else {
        unreachable!("the fixture is flat")
    };
    row.avatar.image_path = Some("/tmp/content-addressed-photo.png".to_owned());

    assert_ne!(before, MailboxRendering::new(&snapshot, "UTC"));
}

#[test]
fn reading_stops_expand_a_conversation_in_place_and_collapse_to_its_representative() {
    let mut snapshot = crate::ui::model::empty_mailbox();
    let first = FlatRow {
        key: "flat-first".to_owned(),
        ..fixture("First")
    };
    let messages = vec![
        ThreadMessage {
            avatar: crate::ui::model::blank_avatar(),
            account: "fixture".to_owned(),
            key: "thread-newest".to_owned(),
            from: "Newest sender".to_owned(),
            date: "2026-07-21".to_owned(),
            preview: String::new(),
            unread: false,
            outgoing: true,
            has_attachment: false,
        },
        ThreadMessage {
            avatar: crate::ui::model::blank_avatar(),
            account: "fixture".to_owned(),
            key: "thread-representative".to_owned(),
            from: "Representative sender".to_owned(),
            date: "2026-07-20".to_owned(),
            preview: String::new(),
            unread: true,
            outgoing: false,
            has_attachment: false,
        },
    ];
    let thread = ThreadRow {
        avatar: crate::ui::model::blank_avatar(),
        account: "fixture".to_owned(),
        thread_id: "thread".to_owned(),
        latest_key: "thread-representative".to_owned(),
        subject: "Conversation".to_owned(),
        latest_from: "Representative sender".to_owned(),
        latest_date: "2026-07-20".to_owned(),
        message_count: 2,
        unread_count: 1,
        has_attachment: false,
        preview: String::new(),
        messages,
    };
    let last = FlatRow {
        key: "flat-last".to_owned(),
        ..fixture("Last")
    };
    let expanded_key = ThreadKey::of(&thread);
    snapshot.rows = vec![
        SnapshotRow::Flat { row: first },
        SnapshotRow::Thread { row: thread },
        SnapshotRow::Flat { row: last },
    ];

    let collapsed = crate::ui::model::readable_stops(&snapshot, &HashSet::new());
    assert_eq!(
        collapsed
            .iter()
            .map(|message| message.key.as_str())
            .collect::<Vec<_>>(),
        ["flat-first", "thread-representative", "flat-last"]
    );

    let expanded = HashSet::from([expanded_key]);
    let open = crate::ui::model::readable_stops(&snapshot, &expanded);
    assert_eq!(
        open.iter()
            .map(|message| message.key.as_str())
            .collect::<Vec<_>>(),
        [
            "flat-first",
            "thread-newest",
            "thread-representative",
            "flat-last"
        ]
    );
    assert_eq!(open[1].subject, "Conversation");
}

/// Every `GtkLabel` under `root`, in tree order: the widgets that actually carry the row's text.
pub(crate) fn labels(root: &gtk::Widget) -> Vec<gtk::Label> {
    let mut found = Vec::new();
    if let Some(label) = root.downcast_ref::<gtk::Label>() {
        found.push(label.clone());
    }
    let mut child = root.first_child();
    while let Some(node) = child {
        found.extend(labels(&node));
        child = node.next_sibling();
    }
    found
}

/// What the row actually *shows*.
///
/// `ActionRow::title()` returns the string we handed it whatever happens to the label, so it
/// answers "did we ask for this text", not "is this text on screen". A markup-parsed row with
/// a bare ampersand renders **empty** while `title()` still reads back in full, so asserting
/// on the property is a green light for a blank row.
pub(crate) fn rendered_labels(root: &gtk::Widget) -> Vec<String> {
    labels(root)
        .iter()
        .map(|label| label.text().to_string())
        .collect()
}

/// Asserts every `GtkListBoxRow` under `root` is in a `GtkListBox`.
///
/// A row parented to a plain box renders, so no rendering assertion sees it; but GTK's focus
/// walk reaches it and `gtk_list_box_row_grab_focus` fails its own precondition, so the row and
/// the control it carries are skipped. `AdwPreferencesGroup` supplies the list; an `AdwActionRow`
/// or `AdwSwitchRow` appended to a `GtkBox` does not.
pub(crate) fn every_row_belongs_to_a_list(root: &gtk::Widget) {
    if let Some(row) = root.downcast_ref::<gtk::ListBoxRow>() {
        assert!(
            row.parent()
                .is_some_and(|parent| parent.is::<gtk::ListBox>()),
            "a row must sit in a list box, not a plain container: {:?} in {:?}",
            root.type_(),
            row.parent().map(|parent| parent.type_())
        );
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        every_row_belongs_to_a_list(&widget);
        child = widget.next_sibling();
    }
}

/// Runs `build`, returning every GLib log record it emitted.
///
/// The rendering assertions below cannot see the defect this guards: a row built as
/// `.subtitle(…).use_markup(false)` still *reads* correctly, because libadwaita re-applies
/// the labels when the flag flips. What it leaves behind is a `Failed to set text … from
/// markup` warning per row, on every sender or subject with an ampersand: noise in the
/// diagnostic log a user attaches to a support request. The warning is the only observable,
/// so the test has to read it.
///
/// **This does not nest, and it cannot be made to.** GLib offers no way to read the handler
/// currently installed, so the `log_unset_default_handler` below restores *GLib's* default rather
/// than whatever was there before; any handler installed earlier stops firing, silently, and a
/// test relying on one goes green while the thing it watches for is still happening. Diagnose a
/// GLib record with `scripts/dev/gtk-trace.sh` instead of a handler installed beside this one.
pub(crate) fn glib_records<T>(build: impl FnOnce() -> T) -> (T, Vec<String>) {
    let records = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = std::sync::Arc::clone(&records);
    gtk::glib::log_set_default_handler(move |_domain, _level, message| {
        sink.lock().expect("log sink").push(message.to_owned());
    });
    let value = build();
    gtk::glib::log_unset_default_handler();
    let captured = records.lock().expect("log sink").clone();
    (value, captured)
}

/// The crate's one GTK test: see [`super::thread_tests`] for why there is exactly one.
#[test]
fn gtk_rows_composer_and_required_modals_obey_their_contracts() {
    gtk::init().expect("GTK test requires a display (CI runs it under Xvfb)");
    let (row_sender, _row_receiver) = relm4::channel::<AppInput>();
    super::thread_tests::conversation_rows_expand_and_unread_mail_is_bold();
    super::thread_tests::every_mail_row_formats_its_timestamp();
    super::thread_tests::a_conversation_reports_what_the_reader_asked_for();
    super::thread_tests::the_apps_glyphs_are_bundled_with_the_app();
    super::thread_tests::a_rerender_rebuilds_only_the_row_that_changed();
    super::thread_tests::a_removed_row_leaves_its_neighbours_widgets_alone();
    super::thread_tests::mail_arriving_at_the_top_does_not_rebuild_the_list_below_it();
    crate::ui::mailbox_progressive::tests::a_new_folder_builds_only_the_visible_rows_synchronously(
    );
    crate::ui::mail_actions::tests::the_action_menus_dispatch_the_message_and_thread_the_user_chose(
    );
    crate::ui::mail_actions::tests::permanent_delete_is_confirmed_before_it_dispatches();
    crate::ui::composer_draft::widget_tests::the_draft_question_discards_only_on_the_discard_button(
    );
    crate::ui::composer_draft::widget_tests::each_navigation_gets_its_own_answer();
    crate::ui::composer_attach::tests::the_drop_target_listens_ahead_of_the_web_view();
    crate::ui::calendar::attendees::tests::attendee_rows_never_parse_a_name_as_markup();
    crate::ui::calendar::widget_tests::
        the_create_drag_owns_the_primary_pointer_before_event_buttons();
    crate::ui::calendar::widget_tests::recentring_releases_the_scene_before_value_notification();
    crate::ui::calendar::widget_tests::a_click_on_an_event_does_not_park_focus_on_the_grid();
    crate::ui::calendar::dialog_tests::neither_series_question_states_its_title_twice();
    crate::ui::calendar::dialog_tests::the_editor_never_pre_empts_the_scope_question();
    crate::ui::reading::attachment_tests::the_reading_header_formats_its_timestamp();
    crate::ui::reading::attachment_tests::an_attachment_name_is_never_parsed_as_markup();
    crate::ui::reading::attachment_tests::an_attachment_button_still_reads_as_its_verb();
    crate::ui::modal::tests::a_modal_renders_its_title_in_native_chrome_only();
    crate::ui::avatar::tests::avatars_and_unread_dots_are_presentational();
    crate::ui::settings::tests::a_closed_settings_window_is_not_on_screen();
    crate::ui::settings::allodia::tests::the_card_names_the_account_by_address_and_offers_a_way_out(
    );
    crate::ui::settings::allodia::tests::a_nameless_account_gets_no_empty_second_line();
    crate::ui::settings::allodia::tests::each_state_offers_exactly_one_action();
    crate::ui::settings::allodia::tests::signed_out_offers_creating_as_well_as_signing_in();
    crate::ui::settings::allodia::tests::a_signed_in_account_can_be_managed_deleted_and_left();
    crate::ui::settings::allodia::tests::neither_the_address_nor_the_failure_is_parsed_as_markup();
    crate::ui::settings::allodia::tests::every_card_row_is_reachable_from_the_keyboard();
    crate::ui::settings::account_sync_mode::tests::
        the_control_offers_three_positions_and_holds_the_one_in_force();
    crate::ui::settings::account_sync_mode::tests::
        the_description_explains_only_the_position_in_force();
    crate::ui::settings::account_sync_mode::tests::choosing_a_position_asks_for_it_once();
    crate::ui::settings::account_sync_mode::tests::
        pressing_the_position_already_in_force_asks_for_nothing();
    crate::ui::settings::account_sync_mode::tests::the_control_is_reachable_from_the_keyboard();
    crate::ui::settings::allodia_sync::tests::
        a_grant_that_predates_the_feature_offers_the_one_thing_that_fixes_it();
    crate::ui::settings::allodia_sync::tests::a_revoked_grant_says_they_are_signed_out();
    crate::ui::settings::allodia_sync::tests::
        a_failure_that_says_nothing_about_the_grant_offers_no_remedy();
    crate::ui::settings::allodia_sync::tests::every_health_row_is_reachable_from_the_keyboard();
    crate::ui::settings::about::assert_about_page_states_version_support_and_attributions();
    crate::ui::settings::general::assert_the_appearance_row_shows_the_stored_choice();
    crate::ui::settings::accounts::tests::
        an_expired_password_is_replaced_without_removing_the_account();
    connectivity::the_banners_render_the_snapshot_and_keep_the_remedy_actionable();
    crate::ui::unfiled_copy::tests::
        the_unfiled_copy_question_offers_both_answers_and_blocks_double_answers();
    // The ampersand has to sit in **both** halves. A property builder applies `use-markup` in
    // GObject's order, not the written one: it landed after `title` but before `subtitle`,
    // so a fixture with an ampersand only in the subject stays silent while every real
    // "Allodia Mail & Calendar" sender warns. Covering both makes the check order-proof.
    let (widget, records) = glib_records(|| {
        flat_row(
            &fixture_from("Research & Development", "Allodia Mail & Calendar"),
            false,
            "UTC",
            &row_sender,
        )
    });
    assert!(
        !records.iter().any(|line| line.contains("from markup")),
        "building a row must not parse the server's text as markup: {records:?}"
    );
    assert!(!widget.uses_markup());
    assert_eq!(widget.title(), "Research & Development");
    // The server's text has to reach the screen intact: an ampersand is not an entity, and a
    // markup-shaped subject is shown, not applied.
    let shown = rendered_labels(widget.upcast_ref::<gtk::Widget>());
    assert!(
        shown.iter().any(|text| text == "Research & Development"),
        "the subject must render as itself, not blank: {shown:?}"
    );
    assert!(
        shown.iter().any(|text| text == "Allodia Mail & Calendar"),
        "the sender subtitle must render in full: {shown:?}"
    );
    assert!(
        shown.iter().any(|text| text == "2026-07-20"),
        "the row's date must render beside it: {shown:?}"
    );

    let hostile = flat_row(&fixture("<b>Wire transfer</b>"), false, "UTC", &row_sender);
    let hostile_shown = rendered_labels(hostile.upcast_ref::<gtk::Widget>());
    assert!(
        hostile_shown
            .iter()
            .any(|text| text == "<b>Wire transfer</b>"),
        "a markup-shaped subject must be shown verbatim, never styled: {hostile_shown:?}"
    );

    let (action_sender, action_receiver) = relm4::channel::<AppInput>();
    let actionable = flat_row(&fixture("Quarterly planning"), false, "UTC", &action_sender);
    actionable
        .activatable_widget()
        .and_downcast::<gtk::Button>()
        .expect("assistive technology needs a native row action")
        .emit_clicked();
    assert!(matches!(
        action_receiver.recv_sync(),
        Some(AppInput::OpenThreadMessage(message)) if message.key == "1"
    ));

    folder_pane::the_pane_draws_every_account_its_folders_and_its_counts();
    folder_pane::a_server_named_row_is_never_parsed_as_markup();
    folder_pane::every_role_icon_resolves_to_a_real_glyph();
    folder_pane::only_an_unreachable_account_gets_the_warning();
    folder_pane::the_pane_marks_where_the_core_says_we_are();
    folder_pane::moving_the_selection_reuses_the_pane();
    folder_pane::an_optimistic_click_is_not_undone_by_the_previous_snapshot();
    folder_pane::folder_rows_expose_their_navigation_as_a_semantic_action();

    destinations::every_destination_icon_resolves_to_a_real_glyph();
    destinations::the_switcher_navigates_on_a_press_and_stays_quiet_when_the_model_moves();
    destinations::the_switcher_is_pinned_below_the_accounts_and_never_scrolls_with_them();

    contacts::the_list_draws_a_letter_per_section_and_a_badge_only_for_a_merge();
    contacts::a_contacts_own_text_is_never_parsed_as_markup();
    contacts::the_detail_names_the_accounts_only_for_a_merge_and_says_it_is_read_only();
    contacts::the_pane_swaps_between_people_an_empty_state_and_the_placeholder();
    contacts::activating_a_person_opens_them_by_id();
    contacts::the_write_affordances_appear_only_where_a_write_could_land();

    composer_header::a_new_message_keeps_cc_and_bcc_behind_the_chevron();
    composer_header::a_reply_all_opens_with_its_cc_on_screen();
    composer_header::a_mail_link_opens_with_its_bcc_on_screen();

    recipients::a_seeded_field_draws_every_address_as_a_pill();
    recipients::typing_edits_only_the_trailing_token();
    recipients::removing_a_pill_keeps_what_is_still_being_typed();
    recipients::accepting_a_suggestion_inserts_the_address_bare();
    recipients::the_key_handler_runs_before_the_entrys_own();
    recipients::a_recipients_own_text_is_never_parsed_as_markup();
    recipients::an_ampersand_survives_into_the_pill_and_the_suggestion();

    search::the_filter_and_the_horizon_are_shown_for_a_search_and_nothing_else();
    search::the_filter_names_the_folder_it_would_narrow_to();
    search::moving_the_filter_dispatches_only_the_side_that_became_active();
    search::rendering_the_cores_state_dispatches_nothing_back();
    search::a_render_behind_the_typing_leaves_the_field_alone();
    search::escape_leaves_search();
    search::typing_is_debounced_on_the_contracts_beat();

    signatures::a_signatures_own_text_is_never_parsed_as_markup();

    invitation::an_invitations_own_text_is_never_parsed_as_markup();
    invitation::an_account_that_cannot_answer_says_so_instead_of_greying_the_buttons();
    invitation::the_note_and_the_tick_appear_only_where_the_transport_carries_them();
    invitation::a_cancelled_or_superseded_card_states_itself_and_offers_no_answer();
    invitation::an_unread_calendar_withholds_both_the_count_and_the_grid();
    invitation::a_settling_write_is_reported_and_a_settled_one_is_not();
    invitation::the_reply_question_names_the_recipient_and_withholds_the_status_code();

    let application = adw::Application::builder()
        .application_id(format!("{}.composer-test", crate::l10n::APP_ID))
        .build();
    application
        .register(None::<&gtk::gio::Cancellable>)
        .expect("register test application");
    let window = adw::ApplicationWindow::new(&application);
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let pane = ComposerPane::new();
    let request = ComposeRequest {
        kind: ComposeKind::Reply,
        account: Some("fixture".to_owned()),
        key: Some("message".to_owned()),
        initial_to: "recipient@example.test".to_owned(),
        initial_cc: String::new(),
        initial_bcc: String::new(),
        subject: "Re: fixture".to_owned(),
        initial_body: None,
        quote: None,
        initial_from: Some("fixture".to_owned()),
        seeds_signature: true,
    };
    pane.show(
        42,
        &request,
        &[("fixture".to_owned(), "sender@example.test".to_owned())],
        None,
        &window,
        sender,
    );

    assert!(pane.is_active(42));
    assert!(pane.widget().first_child().is_some());
    pane.teardown();
    assert!(pane.widget().first_child().is_none());

    crate::ui::setup_widget_tests::the_setup_window_offers_each_route_its_own_surface();
    crate::ui::setup_onboarding_tests::the_first_account_screen_offers_an_allodia_account(&window);
}
