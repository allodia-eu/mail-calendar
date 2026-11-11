//! The agent against real widgets. [`run`] is called from the crate's one GTK test; the socket's
//! permissions need no display and are a test of their own.

use std::{cell::Cell, os::unix::fs::PermissionsExt, rc::Rc, time::Instant};

use adw::prelude::*;
use gtk::glib;
use serde_json::{Value, json};

use super::protocol::{handle, verb_names};
use crate::ui::icons::REPLY;

fn ask(request: &Value) -> Value {
    glib::MainContext::default().block_on(handle(&request.to_string()))
}

fn ok(request: &Value) -> Value {
    let reply = ask(request);
    assert_eq!(reply["ok"], true, "{request} answered {reply}");
    reply
}

fn code(request: &Value) -> String {
    let reply = ask(request);
    assert_eq!(reply["ok"], false, "{request} answered {reply}");
    reply["code"].as_str().unwrap_or_default().to_owned()
}

fn only(request: &Value) -> u64 {
    let reply = ok(request);
    let elements = reply["elements"].as_array().expect("a list of elements");
    assert_eq!(elements.len(), 1, "{request} matched {reply}");
    elements[0]["h"].as_u64().expect("a handle")
}

struct Fixture {
    window: gtk::Window,
    clicks: Rc<Cell<u32>>,
    rows: Rc<Cell<u32>>,
    entry: gtk::Entry,
}

fn fixture() -> Fixture {
    let clicks = Rc::new(Cell::new(0));
    let rows = Rc::new(Cell::new(0));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.append(&gtk::Label::new(Some("Inbox")));
    let archive = gtk::Button::with_label("Archive");
    archive.connect_clicked({
        let clicks = clicks.clone();
        move |_| clicks.set(clicks.get() + 1)
    });
    content.append(&archive);
    content.append(&gtk::Button::from_icon_name(REPLY));
    let entry = gtk::Entry::new();
    entry.set_widget_name("search");
    content.append(&entry);
    let list = gtk::ListBox::new();
    let work = adw::ActionRow::builder()
        .title("Work")
        .activatable(true)
        .build();
    list.append(&work);
    list.append(&adw::ExpanderRow::builder().title("Thread").build());
    list.append(&adw::SwitchRow::builder().title("Dark messages").build());
    list.connect_row_activated({
        let rows = rows.clone();
        move |_, _| rows.set(rows.get() + 1)
    });
    content.append(&list);
    content.append(&gtk::Label::new(Some("Copied")));
    let window = gtk::Window::builder()
        .title("Agent test")
        .default_width(400)
        .default_height(400)
        .child(&content)
        .build();
    window.present();
    let started = Instant::now();
    while !window.is_mapped() || archive.width() == 0 {
        assert!(started.elapsed().as_secs() < 10, "the window never mapped");
        glib::MainContext::default().iteration(true);
    }
    Fixture {
        window,
        clicks,
        rows,
        entry,
    }
}

pub(crate) fn run() {
    let _ = adw::init();
    let fixture = fixture();

    every_verb_is_routed();
    a_widget_is_found_by_what_it_says_and_acted_on();
    a_misspelt_argument_is_refused_rather_than_ignored();
    a_lookup_that_finds_nothing_names_what_is_there(&fixture);
    text_typed_in_is_found_again(&fixture);
    rows_toggles_and_expanders_answer_activation(&fixture);
    the_dump_folds_containers_that_carry_nothing();
    a_handle_to_a_destroyed_widget_says_it_is_gone();
    a_window_draws_its_own_picture();
    an_idle_app_settles();

    fixture.window.destroy();
}

fn every_verb_is_routed() {
    assert_eq!(code(&json!({ "verb": "fly" })), "unknown-verb");
    assert_eq!(code(&json!({ "nothing": 1 })), "bad-request");
    for verb in verb_names() {
        if verb == "quit" {
            continue;
        }
        let reply = ask(&json!({ "verb": verb }));
        assert_ne!(
            reply["code"], "unknown-verb",
            "{verb} is listed but not routed"
        );
    }
}

fn a_widget_is_found_by_what_it_says_and_acted_on() {
    let reply = ok(&json!({ "verb": "query", "text": "Archive" }));
    let archive = &reply["elements"][0];
    assert_eq!(archive["type"], "GtkButton");
    assert_eq!(archive["role"], "button");
    assert_eq!(archive["showing"], true);
    assert_eq!(archive["bounds"].as_array().map(Vec::len), Some(4));
    // A subclass is found by the class it derives from.
    only(&json!({ "verb": "query", "type": "AdwPreferencesRow", "text": "Work" }));
    only(&json!({ "verb": "query", "textLike": "arch", "role": "button" }));
    only(&json!({ "verb": "query", "id": "search" }));
    let reply = only(&json!({ "verb": "query", "icon": REPLY }));
    assert_eq!(
        ok(&json!({ "verb": "read", "h": reply }))["type"],
        "GtkButton"
    );
    let window = ok(&json!({ "verb": "windows" }));
    assert!(
        window["windows"]
            .as_array()
            .is_some_and(|windows| windows.iter().any(|w| w["title"] == "Agent test"))
    );
}

fn a_misspelt_argument_is_refused_rather_than_ignored() {
    assert_eq!(
        code(&json!({ "verb": "query", "txt": "Archive" })),
        "bad-request"
    );
    assert_eq!(
        code(&json!({ "verb": "activate", "h": 1, "x": 0 })),
        "bad-request"
    );
    assert_eq!(
        code(&json!({ "verb": "wait", "text": "Archive", "timeout": 5 })),
        "bad-request"
    );
    assert_eq!(code(&json!({ "verb": "ping", "x": 0 })), "bad-request");
}

fn a_lookup_that_finds_nothing_names_what_is_there(fixture: &Fixture) {
    let reply = ask(&json!({ "verb": "wait", "text": "Archive all", "timeoutMs": 100 }));
    assert_eq!(reply["code"], "timeout");
    assert!(reply["near"].to_string().contains("Archive"), "{reply}");
    let clicks = fixture.clicks.get();
    let h = only(&json!({ "verb": "query", "text": "Archive" }));
    assert_eq!(ok(&json!({ "verb": "activate", "h": h }))["how"], "clicked");
    assert_eq!(fixture.clicks.get(), clicks + 1);
}

fn text_typed_in_is_found_again(fixture: &Fixture) {
    let h = only(&json!({ "verb": "query", "id": "search" }));
    ok(&json!({ "verb": "set-text", "h": h, "text": "invoice" }));
    assert_eq!(fixture.entry.text(), "invoice");
    assert_eq!(ok(&json!({ "verb": "read", "h": h }))["value"], "invoice");
    only(&json!({ "verb": "query", "text": "invoice" }));
    fixture.entry.set_sensitive(false);
    assert_eq!(
        code(&json!({ "verb": "set-text", "h": h, "text": "x" })),
        "disabled"
    );
    assert_eq!(ok(&json!({ "verb": "read", "h": h }))["enabled"], false);
    only(&json!({ "verb": "query", "id": "search", "enabled": false }));
    fixture.entry.set_sensitive(true);
    let label = only(&json!({ "verb": "query", "text": "Inbox" }));
    assert_eq!(
        code(&json!({ "verb": "set-text", "h": label, "text": "x" })),
        "not-editable"
    );
}

fn rows_toggles_and_expanders_answer_activation(fixture: &Fixture) {
    let work = only(&json!({ "verb": "query", "type": "AdwActionRow", "text": "Work" }));
    assert_eq!(
        ok(&json!({ "verb": "activate", "h": work }))["how"],
        "row-activated"
    );
    assert_eq!(fixture.rows.get(), 1);
    assert_eq!(ok(&json!({ "verb": "read", "h": work }))["selected"], true);

    let thread = only(&json!({ "verb": "query", "type": "AdwExpanderRow" }));
    assert_eq!(
        ok(&json!({ "verb": "activate", "h": thread }))["expanded"],
        true
    );

    let dark = only(&json!({ "verb": "query", "type": "AdwSwitchRow" }));
    assert_eq!(ok(&json!({ "verb": "toggle", "h": dark }))["active"], true);
    assert_eq!(
        ok(&json!({ "verb": "toggle", "h": dark, "active": true }))["active"],
        true
    );
    assert_eq!(ok(&json!({ "verb": "read", "h": dark }))["active"], true);
}

fn the_dump_folds_containers_that_carry_nothing() {
    let compact = ok(&json!({ "verb": "dump", "window": "Agent test" }))["text"].to_string();
    // The button says Archive once: the label inside it repeats it and is left out.
    assert_eq!(compact.matches("Archive").count(), 1, "{compact}");
    assert_eq!(compact.matches(REPLY).count(), 1, "{compact}");
    assert!(!compact.contains("GtkBox"), "{compact}");
    let full =
        ok(&json!({ "verb": "dump", "window": "Agent test", "full": true }))["text"].to_string();
    assert!(full.contains("GtkBox"), "{full}");
}

fn a_handle_to_a_destroyed_widget_says_it_is_gone() {
    let h = only(&json!({ "verb": "query", "text": "Copied" }));
    ok(&json!({ "verb": "wait", "text": "Copied", "timeoutMs": 100 }));
    let label = super::tree::widget(&super::protocol::Target { h }).expect("still there");
    label.unparent();
    // Out of the tree, and with this the last reference, so the label is finalised.
    drop(label);
    ok(&json!({ "verb": "wait", "text": "Copied", "absent": true, "timeoutMs": 1000 }));
    assert_eq!(code(&json!({ "verb": "read", "h": h })), "gone");
    assert_eq!(code(&json!({ "verb": "read", "h": 999_999 })), "no-element");
}

fn a_window_draws_its_own_picture() {
    let path = std::env::temp_dir().join(format!("agent-shot-{}.png", std::process::id()));
    let reply = ok(&json!({ "verb": "screenshot", "path": path }));
    assert!(
        reply["width"].as_i64().is_some_and(|width| width > 0),
        "{reply}"
    );
    let bytes = std::fs::read(&path).expect("the picture was written");
    assert!(bytes.starts_with(b"\x89PNG"), "not a PNG");
    std::fs::remove_file(path).ok();
}

fn an_idle_app_settles() {
    let reply = ok(&json!({ "verb": "settle", "quietMs": 100, "capMs": 5000 }));
    assert_eq!(reply["settled"], true, "{reply}");
}

#[test]
fn the_socket_answers_its_owner_alone() {
    let dir = std::env::temp_dir().join(format!("agent-socket-{}", std::process::id()));
    let path = dir.join("agent.sock");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&path, b"left behind by an earlier run").unwrap();
    let _listener = super::listen(&path).expect("binds over a stale file");
    let mode =
        |path: &std::path::Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&path), 0o600);
    assert_eq!(mode(&dir), 0o700);
    std::fs::remove_dir_all(dir).unwrap();
}
