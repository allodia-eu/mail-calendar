//! Finding widgets: the filter every lookup takes, and what a lookup that found nothing offers
//! instead.

use adw::prelude::*;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    protocol::{Answer, Failure, Target},
    tree,
};

/// The most a lookup returns, so a filter that matches every label cannot flood the caller.
const MOST: usize = 200;

#[derive(Deserialize, Default, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct Filter {
    /// A label, title, tooltip or value, whole.
    #[serde(default)]
    text: Option<String>,
    /// Part of one, in any case.
    #[serde(default)]
    text_like: Option<String>,
    /// A GType name; subclasses match too.
    #[serde(default, rename = "type")]
    kind: Option<String>,
    /// The identifier a developer gave it (`gtk::Widget::set_widget_name`), which no person reads.
    #[serde(default)]
    id: Option<String>,
    /// The named icon it shows, for a button with no label.
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    role: Option<String>,
    /// Part of a window's title.
    #[serde(default)]
    window: Option<String>,
    #[serde(default)]
    under: Option<u64>,
    #[serde(default)]
    hidden: bool,
    /// Whether a person could act on it (GTK's sensitivity).
    #[serde(default)]
    enabled: Option<bool>,
    /// A filter some descendant has to match.
    #[serde(default)]
    has: Option<Box<Filter>>,
    /// Only the match at this position, counting from 0.
    #[serde(default)]
    index: Option<usize>,
}

impl Filter {
    fn matches(&self, widget: &gtk::Widget) -> bool {
        if !self.hidden && !widget.is_mapped() {
            return false;
        }
        if let Some(enabled) = self.enabled
            && widget.is_sensitive() != enabled
        {
            return false;
        }
        if let Some(kind) = &self.kind {
            let Some(wanted) = gtk::glib::Type::from_name(kind) else {
                return false;
            };
            if !widget.type_().is_a(wanted) {
                return false;
            }
        }
        if let Some(id) = &self.id
            && tree::id(widget).as_deref() != Some(id)
        {
            return false;
        }
        if let Some(icon) = &self.icon
            && tree::icon(widget).as_deref() != Some(icon)
        {
            return false;
        }
        if let Some(role) = &self.role
            && tree::role(widget) != *role
        {
            return false;
        }
        if self.text.is_some() || self.text_like.is_some() {
            let said = said(widget);
            if let Some(text) = &self.text
                && !said.iter().any(|said| said == text)
            {
                return false;
            }
            if let Some(like) = &self.text_like {
                let like = like.to_lowercase();
                if !said.iter().any(|said| said.to_lowercase().contains(&like)) {
                    return false;
                }
            }
        }
        if let Some(has) = &self.has {
            let mut found = false;
            for child in tree::children(widget) {
                walk(&child, &mut |descendant| {
                    found |= has.matches(descendant);
                    !found
                });
                if found {
                    break;
                }
            }
            if !found {
                return false;
            }
        }
        true
    }

    fn roots(&self) -> Result<Vec<gtk::Widget>, Failure> {
        match self.under {
            Some(h) => Ok(vec![tree::widget(&Target { h })?]),
            None => Ok(windows_titled(self.window.as_deref())),
        }
    }

    /// Whether a lookup had anything to go on beyond where to look.
    fn names_text(&self) -> Option<String> {
        self.text.clone().or_else(|| self.text_like.clone())
    }
}

/// Everything a filter's text is compared with.
fn said(widget: &gtk::Widget) -> Vec<String> {
    let mut said = tree::texts(widget);
    said.extend(widget.tooltip_text().map(|text| text.to_string()));
    said.extend(tree::value(widget));
    said
}

/// What a widget says, and the icon it shows: what makes a match inside another one its echo.
fn said_or_shown(widget: &gtk::Widget) -> Vec<String> {
    let mut said = said(widget);
    said.extend(tree::icon(widget));
    said
}

/// The windows whose title contains `title`, or every window.
pub(super) fn windows_titled(title: Option<&str>) -> Vec<gtk::Widget> {
    tree::toplevels()
        .into_iter()
        .filter(|widget| {
            title.is_none_or(|title| {
                widget
                    .downcast_ref::<gtk::Window>()
                    .and_then(gtk::prelude::GtkWindowExt::title)
                    .is_some_and(|actual| actual.contains(title))
            })
        })
        .collect()
}

/// Visits `widget` and its descendants depth first, until `visit` answers false.
fn walk(widget: &gtk::Widget, visit: &mut dyn FnMut(&gtk::Widget) -> bool) -> bool {
    if !visit(widget) {
        return false;
    }
    for child in tree::children(widget) {
        if !walk(&child, visit) {
            return false;
        }
    }
    true
}

pub(super) fn find(filter: &Filter) -> Result<Vec<gtk::Widget>, Failure> {
    let mut found = Vec::new();
    for root in filter.roots()? {
        walk(&root, &mut |widget| {
            if filter.matches(widget) {
                found.push(widget.clone());
            }
            found.len() < MOST
        });
    }
    let found = without_echoes(&found);
    if let Some(index) = filter.index {
        return Ok(found.into_iter().nth(index).into_iter().collect());
    }
    Ok(found)
}

/// Leaves out a match that only repeats an enclosing match: the label inside a button says the
/// button's text, and the button is the thing a caller means.
fn without_echoes(found: &[gtk::Widget]) -> Vec<gtk::Widget> {
    found
        .iter()
        .filter(|widget| {
            let says = said_or_shown(widget);
            says.is_empty()
                || !found.iter().any(|outer| {
                    outer != *widget && widget.is_ancestor(outer) && {
                        let outer_says = said_or_shown(outer);
                        says.iter().all(|text| outer_says.contains(text))
                    }
                })
        })
        .cloned()
        .collect()
}

pub(super) fn query(filter: &Filter) -> Answer {
    let found = find(filter)?;
    Ok(json!({ "elements": found.iter().map(tree::describe).collect::<Vec<_>>() }))
}

/// What a lookup that found nothing might have meant: on-screen widgets whose text shares a word
/// with the one asked for. The common miss is a label that changed, and this names the new one.
pub(super) fn near(filter: &Filter) -> Value {
    let Some(wanted) = filter.names_text() else {
        return json!([]);
    };
    let words: Vec<String> = wanted
        .to_lowercase()
        .split_whitespace()
        .filter(|word| word.chars().count() > 2)
        .map(str::to_owned)
        .collect();
    let mut near = Vec::new();
    let Ok(roots) = filter.roots() else {
        return json!([]);
    };
    for root in roots {
        walk(&root, &mut |widget| {
            if widget.is_mapped()
                && said(widget).iter().any(|said| {
                    let said = said.to_lowercase();
                    words.iter().any(|word| said.contains(word.as_str()))
                })
            {
                near.push(tree::summary(widget));
            }
            near.len() < 8
        });
    }
    json!(near)
}
