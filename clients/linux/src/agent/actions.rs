//! Acting on a widget the way a person would, through the widget's own API rather than synthesised
//! input: what a click, a keystroke into a field or a press of Enter does to it.

use adw::prelude::*;
use gtk::glib;
use serde::Deserialize;
use serde_json::{Value, json};
use webkit6::prelude::WebViewExt;

use super::{
    protocol::{Answer, Failure, Target},
    tree,
};

/// The widget a handle names, refused when a person could not act on it either.
fn usable(target: &Target) -> Result<gtk::Widget, Failure> {
    let widget = tree::widget(target)?;
    if !widget.is_sensitive() {
        return Err(Failure::new("the element is disabled", "disabled")
            .with("element", tree::describe(&widget)));
    }
    Ok(widget)
}

pub(super) async fn activate(target: &Target) -> Answer {
    let widget = usable(target)?;
    if let Some(row) = widget.downcast_ref::<adw::ExpanderRow>() {
        row.set_expanded(!row.is_expanded());
        return Ok(json!({ "how": "expanded", "expanded": row.is_expanded() }));
    }
    if let Some(expander) = widget.downcast_ref::<gtk::Expander>() {
        expander.set_expanded(!expander.is_expanded());
        return Ok(json!({ "how": "expanded", "expanded": expander.is_expanded() }));
    }
    // A row is chosen and then activated, as a click on it is. GTK activates only a row marked
    // activatable; one that is not is merely chosen, and says so.
    if let Some(row) = widget.downcast_ref::<gtk::ListBoxRow>() {
        let list = row.parent().and_downcast::<gtk::ListBox>();
        let selectable = list
            .as_ref()
            .is_some_and(|list| list.selection_mode() != gtk::SelectionMode::None);
        if let Some(list) = list.as_ref().filter(|_| selectable) {
            list.select_row(Some(row));
        }
        // A click reaches the list, which announces `row-activated`; an action row hears its
        // activation from there too. Activating the row widget itself skips the list.
        if row.is_activatable() {
            let clicked = ClickWatch::on(row);
            match &list {
                Some(list) => list.emit_by_name::<()>("row-activated", &[row]),
                None => {
                    row.activate();
                }
            }
            if let Some(watch) = clicked {
                watch.clicked().await;
            }
            return Ok(json!({ "how": "row-activated" }));
        }
        if selectable {
            return Ok(json!({ "how": "selected" }));
        }
        // Activating the row widget would answer true and do nothing: GTK drops it.
        return Err(Failure::new(
            "the row is neither activatable nor selectable: activate the control inside it",
            "not-activatable",
        )
        .with("element", tree::describe(&widget)));
    }
    // A button is clicked at once. Activating one plays its press for a quarter of a second and
    // clicks afterwards, so a reply sent after `activate` would arrive before the click.
    if let Some(button) = widget.downcast_ref::<gtk::Button>() {
        button.emit_clicked();
        return Ok(json!({ "how": "clicked" }));
    }
    if let Some(menu) = widget.downcast_ref::<gtk::MenuButton>() {
        menu.popup();
        return Ok(json!({ "how": "opened" }));
    }
    if widget.activate() {
        return Ok(json!({ "how": "activated" }));
    }
    Err(Failure::new(
        "the element has no activation of its own: activate the button inside it, or click its \
         bounds with real pointer input",
        "not-activatable",
    )
    .with("element", tree::describe(&widget)))
}

/// The button an action row activates for its row, watched for its click. GTK reaches it through
/// the button's mnemonic, which plays its press for a quarter of a second before it clicks, so a
/// reply sent at once would arrive before the row's action had started: the next request could
/// act on the screen the action is about to replace.
struct ClickWatch {
    button: gtk::Button,
    handler: glib::SignalHandlerId,
    fired: std::rc::Rc<std::cell::Cell<bool>>,
}

impl ClickWatch {
    fn on(row: &gtk::ListBoxRow) -> Option<Self> {
        let button = row
            .downcast_ref::<adw::ActionRow>()?
            .activatable_widget()?
            .downcast::<gtk::Button>()
            .ok()?;
        let fired = std::rc::Rc::new(std::cell::Cell::new(false));
        let handler = button.connect_clicked({
            let fired = fired.clone();
            move |_| fired.set(true)
        });
        Some(Self {
            button,
            handler,
            fired,
        })
    }

    /// Returns once the button has clicked, or after a second if it never does.
    async fn clicked(self) {
        let started = std::time::Instant::now();
        while !self.fired.get() && started.elapsed() < std::time::Duration::from_secs(1) {
            glib::timeout_future(std::time::Duration::from_millis(10)).await;
        }
        self.button.disconnect(self.handler);
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SetText {
    h: u64,
    text: String,
    /// Press Enter afterwards, as a person submitting the field would.
    #[serde(default)]
    activate: bool,
}

pub(super) fn set_text(args: &SetText) -> Answer {
    let widget = usable(&Target { h: args.h })?;
    let read_only = || {
        Err(Failure::new("the element is read-only", "not-editable")
            .with("element", tree::describe(&widget)))
    };
    if let Some(editable) = widget.dynamic_cast_ref::<gtk::Editable>() {
        if !editable.is_editable() {
            return read_only();
        }
        editable.set_text(&args.text);
        if args.activate {
            match widget.downcast_ref::<adw::EntryRow>() {
                // Enter on a row that shows an apply button applies it, and that is all such a
                // row answers: its other signal would set the text and commit nothing.
                Some(row) if row.shows_apply_button() => row.emit_by_name::<()>("apply", &[]),
                Some(row) => row.emit_by_name::<()>("entry-activated", &[]),
                None => {
                    widget.activate();
                }
            }
        }
        return Ok(json!({}));
    }
    if let Some(view) = widget.downcast_ref::<gtk::TextView>() {
        if !view.is_editable() {
            return read_only();
        }
        view.buffer().set_text(&args.text);
        return Ok(json!({}));
    }
    Err(Failure::new(
        "the element holds no text: query for role text-box, or type GtkEditable",
        "not-editable",
    )
    .with("element", tree::describe(&widget)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Toggle {
    h: u64,
    #[serde(default)]
    active: Option<bool>,
}

pub(super) fn toggle(args: &Toggle) -> Answer {
    let widget = usable(&Target { h: args.h })?;
    let set = |current: bool| args.active.unwrap_or(!current);
    if let Some(toggle) = widget.downcast_ref::<gtk::ToggleButton>() {
        toggle.set_active(set(toggle.is_active()));
        return Ok(json!({ "active": toggle.is_active() }));
    }
    if let Some(check) = widget.downcast_ref::<gtk::CheckButton>() {
        check.set_active(set(check.is_active()));
        return Ok(json!({ "active": check.is_active() }));
    }
    if let Some(switch) = widget.downcast_ref::<gtk::Switch>() {
        switch.set_active(set(switch.is_active()));
        return Ok(json!({ "active": switch.is_active() }));
    }
    if let Some(row) = widget.downcast_ref::<adw::SwitchRow>() {
        row.set_active(set(row.is_active()));
        return Ok(json!({ "active": row.is_active() }));
    }
    if let Some(row) = widget.downcast_ref::<adw::ExpanderRow>() {
        row.set_expanded(set(row.is_expanded()));
        return Ok(json!({ "active": row.is_expanded() }));
    }
    Err(
        Failure::new("the element has no on and off", "not-toggleable")
            .with("element", tree::describe(&widget)),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Select {
    h: u64,
    index: u32,
}

pub(super) fn select(args: &Select) -> Answer {
    let widget = usable(&Target { h: args.h })?;
    // A position past the end selects nothing and still answers, so it is refused here.
    let within = |model: Option<gtk::gio::ListModel>| {
        let count = model.map_or(0, |model| model.n_items());
        if args.index < count {
            Ok(())
        } else {
            Err(Failure::new(
                format!(
                    "the element has {count} choices, so no choice {}",
                    args.index
                ),
                "no-element",
            ))
        }
    };
    if let Some(combo) = widget.downcast_ref::<adw::ComboRow>() {
        within(combo.model())?;
        combo.set_selected(args.index);
        return Ok(json!({}));
    }
    if let Some(drop_down) = widget.downcast_ref::<gtk::DropDown>() {
        within(drop_down.model())?;
        drop_down.set_selected(args.index);
        return Ok(json!({}));
    }
    if let Some(list) = widget.downcast_ref::<gtk::ListBox>() {
        let index = i32::try_from(args.index)
            .map_err(|_| Failure::new("the index is too large", "bad-request"))?;
        let row = list
            .row_at_index(index)
            .ok_or_else(|| Failure::new(format!("the list has no row {index}"), "no-element"))?;
        list.select_row(Some(&row));
        return Ok(json!({}));
    }
    Err(
        Failure::new("the element offers no choice", "not-selectable")
            .with("element", tree::describe(&widget)),
    )
}

pub(super) fn focus(target: &Target) -> Answer {
    let widget = tree::widget(target)?;
    Ok(json!({ "focused": widget.grab_focus() }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Action {
    name: String,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    h: Option<u64>,
}

pub(super) fn action(args: &Action) -> Answer {
    let target = args
        .target
        .as_deref()
        .map(|text| glib::Variant::parse(None, text))
        .transpose()
        .map_err(|error| {
            Failure::new(
                format!("target is not GVariant text ({error}): write 'text', true or int32 3"),
                "bad-request",
            )
        })?;
    let widget = match args.h {
        Some(h) => tree::widget(&Target { h })?,
        None => tree::toplevels()
            .into_iter()
            .next()
            .ok_or_else(|| Failure::new("no window is open", "no-element"))?,
    };
    widget
        .activate_action(&args.name, target.as_ref())
        .map_err(|_| {
            Failure::new(
                format!("no action {} reachable from there", args.name),
                "no-action",
            )
            .with("actions", actions()["actions"].clone())
        })?;
    Ok(json!({}))
}

/// The actions a caller can name without knowing a widget: the application's and the active
/// window's. A widget's own group is reached by passing its handle to `action`.
pub(super) fn actions() -> Value {
    let mut names: Vec<String> = Vec::new();
    if let Some(application) = gtk::gio::Application::default() {
        names.extend(
            application
                .list_actions()
                .iter()
                .map(|name| format!("app.{name}")),
        );
    }
    if let Some(window) = tree::toplevels()
        .into_iter()
        .next()
        .and_downcast::<gtk::ApplicationWindow>()
    {
        names.extend(
            window
                .list_actions()
                .iter()
                .map(|name| format!("win.{name}")),
        );
    }
    names.sort();
    json!({ "actions": names })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Eval {
    h: u64,
    script: String,
}

pub(super) async fn eval(args: Eval) -> Answer {
    let widget = tree::widget(&Target { h: args.h })?;
    let Some(view) = widget.downcast_ref::<webkit6::WebView>() else {
        return Err(Failure::new(
            "the element is not a web view: query for type WebKitWebView",
            "not-a-web-view",
        ));
    };
    let value = view
        .evaluate_javascript_future(&args.script, None, None)
        .await
        .map_err(|error| Failure::new(format!("the script failed: {error}"), "script-error"))?;
    let value = if value.is_undefined() {
        Value::Null
    } else {
        value
            .to_json(0)
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_else(|| Value::String(value.to_str().to_string()))
    };
    Ok(json!({ "value": value }))
}

pub(super) fn quit() -> Value {
    // The thread serving the caller quits the app once the reply is written, so the caller hears
    // that the request was taken before the process can end.
    super::QUIT_ASKED.store(true, std::sync::atomic::Ordering::SeqCst);
    json!({})
}
