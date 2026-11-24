//! The composer's column, checked against the toolkit. Called from the crate's single `gtk::init`
//! test (see [`super::super::mailbox::tests`]).

use gtk::prelude::*;

use super::{ComposerColumn, MINIMUM_EDITOR_HEIGHT};
use crate::ui::webview::{DocumentKind, SecureWebView};

/// The editor has a height from the moment the column holds it, before the column has measured
/// anything. A WebKitGTK view whose page is laid out while it has no height never paints again
/// on the hardware-accelerated renderer, so a composer opened before its first layout would stay
/// blank.
pub(crate) fn the_editor_has_a_height_before_the_column_is_laid_out() {
    // The composer's own view, on an ephemeral session like every view the app makes: the default
    // session is process-wide and outlives the test's thread.
    let (sender, _receiver) = relm4::channel::<crate::ui::AppInput>();
    let web = SecureWebView::new(DocumentKind::Composer, sender);
    let editor = web.widget().clone();
    let host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    host.append(&editor);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&host);
    let _column = ComposerColumn::new(&content, &host, &editor);
    // Whole logical pixels, as the column sets them.
    #[allow(clippy::cast_possible_truncation)]
    let minimum = MINIMUM_EDITOR_HEIGHT as i32;
    assert!(
        editor.height_request() >= minimum,
        "the editor starts {} tall, below the {minimum} it is drawn at before the column measures",
        editor.height_request()
    );
}
