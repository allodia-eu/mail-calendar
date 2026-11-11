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

pub(super) fn activate(target: &Target) -> Answer {
    let widget = tree::widget(target)?;
    if !widget.is_sensitive() {
        return Err(Failure::new("the element is insensitive", "insensitive")
            .with("element", tree::describe(&widget)));
    }
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
            match &list {
                Some(list) => list.emit_by_name::<()>("row-activated", &[row]),
                None => {
                    row.activate();
                }
            }
            return Ok(json!({ "how": "row-activated" }));
        }
        if selectable {
            return Ok(json!({ "how": "selected" }));
        }
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
    let widget = tree::widget(&Target { h: args.h })?;
    if let Some(editable) = widget.dynamic_cast_ref::<gtk::Editable>() {
        editable.set_text(&args.text);
        if args.activate {
            // An entry row submits through its own signal; an entry through `activate`.
            if let Some(row) = widget.downcast_ref::<adw::EntryRow>() {
                row.emit_by_name::<()>("entry-activated", &[]);
            } else {
                widget.activate();
            }
        }
        return Ok(json!({}));
    }
    if let Some(view) = widget.downcast_ref::<gtk::TextView>() {
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
    let widget = tree::widget(&Target { h: args.h })?;
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
    let widget = tree::widget(&Target { h: args.h })?;
    if let Some(combo) = widget.downcast_ref::<adw::ComboRow>() {
        combo.set_selected(args.index);
        return Ok(json!({}));
    }
    if let Some(drop_down) = widget.downcast_ref::<gtk::DropDown>() {
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
    if let Some(application) = gtk::gio::Application::default() {
        // After the reply, so the caller hears that the request was taken.
        glib::idle_add_local_once(move || application.quit());
    }
    json!({})
}
