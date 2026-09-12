//! What belongs to the mail window's top row.

use adw::prelude::*;

use super::MailToolbar;
use crate::ui::{AppInput, search::SearchBar};

pub(crate) fn new_mail_and_search_belong_to_the_window_toolbar() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let search = SearchBar::new(&sender);
    let toolbar = MailToolbar::new(&sender, search.entry());

    assert_eq!(
        toolbar.widget().title_widget().as_ref(),
        Some(search.entry().upcast_ref::<gtk::Widget>()),
        "the search field is the toolbar's centred item"
    );
    toolbar.compose.emit_clicked();
    assert!(matches!(receiver.recv_sync(), Some(AppInput::BeginNew)));
}
