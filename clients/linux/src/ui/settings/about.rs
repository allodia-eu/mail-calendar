//! Settings → About: which release this is, where to ask for help, and whose work it is built on.

use adw::prelude::*;
use mailcal_bindings::{AboutPlatform, about_info};

use super::{PageContext, group, page_box};
use crate::l10n;

pub(super) fn about(ctx: &PageContext) -> gtk::Box {
    about_page(&ctx.window)
}

/// The page itself, over nothing but the window a launch is parented to; so a test can build it
/// without booting a core.
fn about_page(window: &gtk::Window) -> gtk::Box {
    // Version, support address and attributions come from the core, so every client says the
    // same thing; only the labels around them are this client's.
    let info = about_info(AboutPlatform::Linux);
    let content = page_box(l10n::settings_category_about());

    // A group draws no header while it holds no rows, so the version is a row rather than the
    // group's description; otherwise the app's own name never reaches the page.
    let identity = group(l10n::app_title(), "");
    identity.add(&plain_row(&l10n::about_version(&info.version)));
    content.append(&identity);

    let support = group(
        l10n::about_support_heading(),
        l10n::about_support_description(),
    );
    let row = plain_row(&info.support_url);
    let open = gtk::Button::with_label(l10n::about_support_action());
    let window = window.clone();
    let url = info.support_url.clone();
    open.connect_clicked(move |_| {
        gtk::UriLauncher::new(&url).launch(Some(&window), gtk::gio::Cancellable::NONE, |_| {});
    });
    open.set_valign(gtk::Align::Center);
    row.add_suffix(&open);
    row.set_activatable_widget(Some(&open));
    support.add(&row);
    content.append(&support);

    let attributions = group(
        l10n::about_attributions_heading(),
        l10n::about_attributions_description(),
    );
    for item in &info.attributions {
        let row = plain_row(&item.name);
        row.set_subtitle(&item.license);
        attributions.add(&row);
    }
    content.append(&attributions);
    content
}

/// A row whose text is plain: a licence expression carries `AND`, and a title parsed as Pango
/// markup would swallow anything shaped like a tag.
fn plain_row(title: &str) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_use_markup(false);
    row.set_title(title);
    row
}

/// Everything About draws has to be *on the page*: the release the core reports, the address a
/// support answer would name, and every attribution; a notice nobody can read is not a notice.
///
/// A function rather than a `#[test]` for the reason [`crate::ui::mailbox`]'s conversation tests
/// give: GTK initialises once, on one thread, and the crate keeps a single GTK test.
#[cfg(test)]
pub(crate) fn assert_about_page_states_version_support_and_attributions() {
    use crate::ui::mailbox::rendered_labels;

    let info = about_info(AboutPlatform::Linux);
    let window = gtk::Window::new();
    let page = about_page(&window);
    window.set_child(Some(&page));
    window.present();
    crate::ui::mailbox::tests::every_row_belongs_to_a_list(window.upcast_ref::<gtk::Widget>());
    let shown = rendered_labels(page.upcast_ref::<gtk::Widget>());

    assert!(
        shown.iter().any(|text| text == crate::l10n::app_title()),
        "the app's own name has to be on its About page: {shown:?}"
    );
    assert!(
        shown.iter().any(|text| text.contains(&info.version)),
        "the page must state the version the core reports ({}): {shown:?}",
        info.version
    );
    assert!(
        shown.iter().any(|text| text == &info.support_url),
        "the support address must be readable, not only behind a button: {shown:?}"
    );
    assert!(!info.attributions.is_empty());
    for item in &info.attributions {
        assert!(
            shown.iter().any(|text| text == &item.name),
            "{} is attributed nowhere: {shown:?}",
            item.name
        );
        assert!(
            shown.iter().any(|text| text == &item.license),
            "{} is named without its licence: {shown:?}",
            item.name
        );
    }
}
