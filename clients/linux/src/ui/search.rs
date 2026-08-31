//! Linux mail-search state and GTK chrome.

pub(crate) mod bar;
mod model;

pub(crate) use bar::SearchBar;
pub(crate) use model::{QueryChange, SearchState};
