//! The widget tree as the agent reports it: handles, what a widget says, and the indented dump.

use std::{cell::RefCell, collections::HashMap, fmt::Write as _};

use adw::prelude::*;
use gtk::glib;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::protocol::{Answer, Failure, Target};

thread_local! {
    static HANDLES: RefCell<Handles> = RefCell::new(Handles::default());
}

/// Handles stay the same for a widget's whole life, so a caller can query once and act later.
#[derive(Default)]
struct Handles {
    next: u64,
    by_widget: HashMap<usize, u64>,
    widgets: HashMap<u64, glib::WeakRef<gtk::Widget>>,
}

pub(super) fn handle(widget: &gtk::Widget) -> u64 {
    HANDLES.with_borrow_mut(|handles| {
        let key = widget.as_ptr() as usize;
        if let Some(h) = handles.by_widget.get(&key).copied()
            && handles
                .widgets
                .get(&h)
                .and_then(glib::WeakRef::upgrade)
                .as_ref()
                == Some(widget)
        {
            return h;
        }
        handles.next += 1;
        let h = handles.next;
        handles.by_widget.insert(key, h);
        handles.widgets.insert(h, widget.downgrade());
        h
    })
}

/// The widget a handle names, or why there is none.
pub(super) fn widget(target: &Target) -> Result<gtk::Widget, Failure> {
    HANDLES.with_borrow(|handles| match handles.widgets.get(&target.h) {
        None => Err(Failure::new(
            format!("no element {}", target.h),
            "no-element",
        )),
        Some(weak) => weak.upgrade().ok_or_else(|| {
            Failure::new(format!("element {} is gone: query again", target.h), "gone")
        }),
    })
}

/// Every window, the active one first: what a caller looks through when it names none.
pub(super) fn toplevels() -> Vec<gtk::Widget> {
    let mut windows: Vec<gtk::Widget> = gtk::Window::list_toplevels()
        .into_iter()
        .filter(gtk::prelude::WidgetExt::is_visible)
        .collect();
    windows.sort_by_key(|window| {
        !window
            .downcast_ref::<gtk::Window>()
            .is_some_and(gtk::prelude::GtkWindowExt::is_active)
    });
    windows
}

pub(super) fn windows() -> Value {
    let windows: Vec<Value> = toplevels()
        .iter()
        .map(|widget| {
            let window = widget.downcast_ref::<gtk::Window>();
            json!({
                "h": handle(widget),
                "type": widget.type_().name(),
                "title": window.and_then(gtk::prelude::GtkWindowExt::title).map(|t| t.to_string()),
                "width": widget.width(),
                "height": widget.height(),
                "active": window.is_some_and(gtk::prelude::GtkWindowExt::is_active),
                "modal": window.is_some_and(gtk::prelude::GtkWindowExt::is_modal),
            })
        })
        .collect();
    json!({ "windows": windows })
}

/// The children of a widget in the order they are drawn.
pub(super) fn children(widget: &gtk::Widget) -> impl Iterator<Item = gtk::Widget> {
    std::iter::successors(widget.first_child(), gtk::Widget::next_sibling)
}

/// What a widget says to a person: its label, title or subtitle. A tooltip and an editable's value
/// are reported beside these rather than among them.
pub(super) fn texts(widget: &gtk::Widget) -> Vec<String> {
    let mut texts: Vec<String> = Vec::new();
    let mut push = |text: Option<glib::GString>| {
        if let Some(text) = text.filter(|text| !text.is_empty()) {
            texts.push(text.to_string());
        }
    };
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        push(Some(label.text()));
    } else if let Some(inscription) = widget.downcast_ref::<gtk::Inscription>() {
        push(inscription.text());
    } else if let Some(button) = widget.downcast_ref::<gtk::Button>() {
        push(button.label());
    } else if let Some(check) = widget.downcast_ref::<gtk::CheckButton>() {
        push(check.label());
    } else if let Some(menu) = widget.downcast_ref::<gtk::MenuButton>() {
        push(menu.label());
    } else if let Some(content) = widget.downcast_ref::<adw::ButtonContent>() {
        push(Some(content.label()));
    } else if let Some(title) = widget.downcast_ref::<adw::WindowTitle>() {
        push(Some(title.title()));
        push(Some(title.subtitle()));
    } else if let Some(page) = widget.downcast_ref::<adw::StatusPage>() {
        push(page.title().into());
        push(page.description());
    } else if let Some(banner) = widget.downcast_ref::<adw::Banner>() {
        push(Some(banner.title()));
    } else if let Some(alert) = widget.downcast_ref::<adw::AlertDialog>() {
        push(Some(alert.heading().unwrap_or_default()));
        push(Some(alert.body()));
    } else if let Some(dialog) = widget.downcast_ref::<adw::Dialog>() {
        push(Some(dialog.title()));
    } else if let Some(window) = widget.downcast_ref::<gtk::Window>() {
        push(window.title());
    }
    if let Some(row) = widget.downcast_ref::<adw::PreferencesRow>() {
        push(Some(row.title()));
        if let Some(row) = widget.downcast_ref::<adw::ActionRow>() {
            push(row.subtitle());
        } else if let Some(row) = widget.downcast_ref::<adw::ExpanderRow>() {
            push(Some(row.subtitle()));
        }
    }
    texts
}

/// The text an editable, entry row or text view holds.
pub(super) fn value(widget: &gtk::Widget) -> Option<String> {
    if let Some(editable) = widget.dynamic_cast_ref::<gtk::Editable>() {
        return Some(editable.text().to_string());
    }
    widget.downcast_ref::<gtk::TextView>().map(|view| {
        let buffer = view.buffer();
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string()
    })
}

/// A widget's state that only some kinds have: on, open, chosen.
fn state(widget: &gtk::Widget, node: &mut Map<String, Value>) {
    let active = if let Some(toggle) = widget.downcast_ref::<gtk::ToggleButton>() {
        Some(toggle.is_active())
    } else if let Some(check) = widget.downcast_ref::<gtk::CheckButton>() {
        Some(check.is_active())
    } else if let Some(switch) = widget.downcast_ref::<gtk::Switch>() {
        Some(switch.is_active())
    } else {
        widget
            .downcast_ref::<adw::SwitchRow>()
            .map(adw::SwitchRow::is_active)
    };
    if let Some(active) = active {
        node.insert("active".to_owned(), Value::Bool(active));
    }
    let expanded = if let Some(row) = widget.downcast_ref::<adw::ExpanderRow>() {
        Some(row.is_expanded())
    } else {
        widget
            .downcast_ref::<gtk::Expander>()
            .map(gtk::Expander::is_expanded)
    };
    if let Some(expanded) = expanded {
        node.insert("expanded".to_owned(), Value::Bool(expanded));
    }
    if let Some(row) = widget.downcast_ref::<gtk::ListBoxRow>()
        && row.is_selected()
    {
        node.insert("selected".to_owned(), Value::Bool(true));
    }
    if let Some(combo) = widget.downcast_ref::<adw::ComboRow>() {
        node.insert("chosen".to_owned(), json!(combo.selected()));
    } else if let Some(drop_down) = widget.downcast_ref::<gtk::DropDown>() {
        node.insert("chosen".to_owned(), json!(drop_down.selected()));
    }
}

/// The role assistive technology is told, in the words AT-SPI uses: `button`, `text-box`.
pub(super) fn role(widget: &gtk::Widget) -> String {
    let role = format!("{:?}", widget.accessible_role());
    let mut words = String::new();
    for (index, character) in role.chars().enumerate() {
        if character.is_uppercase() && index > 0 {
            words.push('-');
        }
        words.push(character.to_ascii_lowercase());
    }
    words
}

/// The widget's name, when someone gave it one: GTK otherwise reports the type's name.
pub(super) fn name(widget: &gtk::Widget) -> Option<String> {
    let name = widget.widget_name();
    (name.as_str() != widget.type_().name()).then(|| name.to_string())
}

pub(super) fn describe(widget: &gtk::Widget) -> Value {
    let mut node = Map::new();
    node.insert("h".to_owned(), json!(handle(widget)));
    node.insert("type".to_owned(), json!(widget.type_().name()));
    if let Some(name) = name(widget) {
        node.insert("name".to_owned(), json!(name));
    }
    node.insert("role".to_owned(), json!(role(widget)));
    let texts = texts(widget);
    if !texts.is_empty() {
        node.insert("text".to_owned(), json!(texts));
    }
    if let Some(tooltip) = widget.tooltip_text() {
        node.insert("tooltip".to_owned(), json!(tooltip.as_str()));
    }
    if let Some(icon) = icon(widget) {
        node.insert("icon".to_owned(), json!(icon));
    }
    if let Some(value) = value(widget) {
        node.insert("value".to_owned(), json!(value));
    }
    state(widget, &mut node);
    if let Some(root) = widget.root()
        && let Some(bounds) = widget.compute_bounds(root.upcast_ref::<gtk::Widget>())
    {
        // Logical pixels in the window's own space, which is what a pointer on that window takes.
        node.insert(
            "bounds".to_owned(),
            json!([
                bounds.x().round(),
                bounds.y().round(),
                bounds.width().round(),
                bounds.height().round()
            ]),
        );
    }
    node.insert("showing".to_owned(), json!(widget.is_mapped()));
    node.insert("sensitive".to_owned(), json!(widget.is_sensitive()));
    if widget.has_focus() {
        node.insert("focused".to_owned(), json!(true));
    }
    Value::Object(node)
}

pub(super) fn read(target: &Target) -> Answer {
    Ok(describe(&widget(target)?))
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct DumpArgs {
    #[serde(default)]
    under: Option<u64>,
    #[serde(default)]
    window: Option<String>,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    full: bool,
}

pub(super) fn dump(args: &DumpArgs) -> Answer {
    let roots = match args.under {
        Some(h) => vec![widget(&Target { h })?],
        None => super::query::windows_titled(args.window.as_deref()),
    };
    let mut text = String::new();
    for root in &roots {
        write_tree(root, 0, args, &Said::default(), &mut text);
    }
    Ok(json!({ "text": text }))
}

/// Whether a widget is worth a line of its own in the compact dump: it says something, is named,
/// holds state, or is something a person acts on.
fn carries_something(widget: &gtk::Widget) -> bool {
    !texts(widget).is_empty()
        || widget.tooltip_text().is_some()
        || value(widget).is_some()
        || name(widget).is_some()
        || widget.is::<gtk::Window>()
        || widget.is::<adw::Dialog>()
        || !matches!(
            widget.accessible_role(),
            gtk::AccessibleRole::Generic
                | gtk::AccessibleRole::Group
                | gtk::AccessibleRole::None
                | gtk::AccessibleRole::Presentation
        )
}

/// What the nearest line above a widget already said, so a label or image that only repeats it
/// is left out of the compact dump: the label inside a button is the button's text.
#[derive(Default)]
struct Said {
    texts: Vec<String>,
    icon: Option<String>,
}

fn echoes(widget: &gtk::Widget, above: &Said) -> bool {
    if !matches!(
        widget.accessible_role(),
        gtk::AccessibleRole::Label | gtk::AccessibleRole::Img
    ) || name(widget).is_some()
        || widget.tooltip_text().is_some()
    {
        return false;
    }
    let icon = icon(widget);
    texts(widget).iter().all(|text| above.texts.contains(text))
        && (icon.is_none() || icon == above.icon)
}

fn write_tree(widget: &gtk::Widget, depth: usize, args: &DumpArgs, above: &Said, out: &mut String) {
    if !args.hidden && !widget.is_mapped() {
        return;
    }
    let shown = args.full || (carries_something(widget) && !echoes(widget, above));
    let mut said = Said::default();
    let above = if shown {
        let _ = writeln!(out, "{}{}", "  ".repeat(depth), summary(widget));
        said.texts = texts(widget);
        said.icon = icon(widget);
        &said
    } else {
        above
    };
    let depth = if shown { depth + 1 } else { depth };
    for child in children(widget) {
        write_tree(&child, depth, args, above, out);
    }
}

/// The named icon a widget shows: an image's, or a button's own or that of the image inside it.
pub(super) fn icon(widget: &gtk::Widget) -> Option<String> {
    let name = if let Some(image) = widget.downcast_ref::<gtk::Image>() {
        image.icon_name()
    } else if let Some(button) = widget.downcast_ref::<gtk::Button>() {
        button.icon_name().or_else(|| {
            button
                .child()
                .and_then(|child| icon(&child).map(glib::GString::from))
        })
    } else if let Some(menu) = widget.downcast_ref::<gtk::MenuButton>() {
        menu.icon_name()
    } else {
        widget
            .downcast_ref::<adw::ButtonContent>()
            .map(adw::ButtonContent::icon_name)
    };
    name.filter(|name| !name.is_empty())
        .map(|name| name.to_string())
}

/// One widget as a dump line: `[12] AdwActionRow #name "Title" role=row value="…" (state)`.
pub(super) fn summary(widget: &gtk::Widget) -> String {
    let mut line = format!("[{}] {}", handle(widget), widget.type_().name());
    if let Some(name) = name(widget) {
        let _ = write!(line, " #{name}");
    }
    for text in texts(widget) {
        let _ = write!(line, " {:?}", one_line(&text));
    }
    let _ = write!(line, " {}", role(widget));
    if let Some(icon) = icon(widget) {
        let _ = write!(line, " icon={icon}");
    }
    if let Some(tooltip) = widget.tooltip_text() {
        let _ = write!(line, " tooltip={:?}", one_line(&tooltip));
    }
    if let Some(value) = value(widget) {
        let _ = write!(line, " value={:?}", one_line(&value));
    }
    let mut node = Map::new();
    state(widget, &mut node);
    for (key, value) in node {
        let _ = write!(line, " {key}={value}");
    }
    if !widget.is_mapped() {
        line.push_str(" (hidden)");
    }
    if !widget.is_sensitive() {
        line.push_str(" (insensitive)");
    }
    if widget.has_focus() {
        line.push_str(" (focused)");
    }
    line
}

/// A text cut to one line of a reasonable length, for a dump read by eye or by a model.
fn one_line(text: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > 120 {
        format!("{}…", flat.chars().take(119).collect::<String>())
    } else {
        flat
    }
}
