use core::fmt;

use serde::{Deserialize, Serialize};

use crate::types::InlineContent;

/// Whether a list is bulleted or sequentially numbered.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListKind {
    /// Unordered, bullet-marked list (`<ul>`).
    Bullet,
    /// Ordered, sequentially numbered list (`<ol>`).
    Ordered,
}

impl ListKind {
    /// The HTML list element tag (`ul` or `ol`) for this kind.
    #[must_use]
    pub const fn html_tag(self) -> &'static str {
        match self {
            Self::Bullet => "ul",
            Self::Ordered => "ol",
        }
    }
}

impl fmt::Debug for ListKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Bullet => "Bullet",
            Self::Ordered => "Ordered",
        })
    }
}

/// A single list item: inline content plus an optional nested sub-list.
///
/// The nested `child` is itself a `List`, so bulleted and ordered lists can
/// nest into one another to any depth.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItem {
    /// Inline content shown on the item's own line.
    pub content: Vec<InlineContent>,
    /// Optional sub-list rendered inside this item.
    #[serde(default)]
    pub child: Option<List>,
}

impl fmt::Debug for ListItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListItem")
            .field("content_len", &self.content.len())
            .field("has_child", &self.child.is_some())
            .finish()
    }
}

/// A bulleted or ordered list whose items may nest further lists.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct List {
    /// Whether items are bulleted or numbered.
    pub kind: ListKind,
    /// Items in order.
    pub items: Vec<ListItem>,
    /// The number an ordered list's first item carries, when it is not 1: a list that carries on
    /// from one a paragraph interrupted, as Word numbers it. A bulleted list ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<u32>,
}

impl List {
    /// The number the first item is shown with, when it is not 1, for an ordered list only.
    #[must_use]
    pub fn first_number(&self) -> Option<u32> {
        match (self.kind, self.start) {
            (ListKind::Ordered, Some(start)) if start != 1 => Some(start),
            _ => None,
        }
    }
}

impl fmt::Debug for List {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("List")
            .field("kind", &self.kind)
            .field("items_len", &self.items.len())
            .field("start", &self.start)
            .finish()
    }
}
