//! Settings → Writing style: the learned styles, which account drafts in which, and the credits
//! line while requests go through Allodia (`docs/ai.md`, `docs/settings.md` row 7).
//!
//! Drawn as Signatures is, because it is the same kind of thing: a named library above a
//! per-account picker, every picker sharing one model, the category built once and then brought to
//! the core rather than rebuilt. What differs is who asks for that. Every change here, a learning
//! run's progress included, signals `Surface::WritingStyle`, so the model pushes each snapshot into
//! the open window through [`Live`] and this page reconciles itself with it; a rebuild would take
//! a half-typed note with it. The rules it draws are `crate::ui::writing_style`.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use mailcal_bindings::{AccountWritingStyleRow, WritingStyleRow, WritingStyleSnapshot};

use super::{
    PageContext, group_titled, page_box,
    signatures::{named_row, slot_choice, slot_selection},
    writing_style_learn, writing_style_reveal,
};
use crate::{
    l10n,
    ui::{mailbox::plain_text_row, writing_style as rules},
};

type Redraw = Box<dyn Fn(&WritingStyleSnapshot)>;

/// What a Writing style snapshot redraws in the open Settings window: this category and every
/// credits line. Each entry reaches its widgets weakly, so one left over from a closed window does
/// nothing until the next build clears the list.
#[derive(Clone, Default)]
pub(super) struct Live(Rc<RefCell<Vec<Redraw>>>);

impl Live {
    pub(super) fn clear(&self) {
        self.0.borrow_mut().clear();
    }

    fn register(&self, redraw: impl Fn(&WritingStyleSnapshot) + 'static) {
        self.0.borrow_mut().push(Box::new(redraw));
    }

    pub(super) fn redraw(&self, snapshot: &WritingStyleSnapshot) {
        for redraw in self.0.borrow().iter() {
            redraw(snapshot);
        }
    }
}

impl std::fmt::Debug for Live {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Live")
            .field("entries", &self.0.borrow().len())
            .finish()
    }
}

pub(super) fn page(ctx: &PageContext) -> gtk::Box {
    let snapshot = ctx.app.writing_styles();
    let content = page_box(l10n::settings_category_writing_style());
    content.append(&paragraph(l10n::writing_style_intro()));
    // Both present from the start, one shown: the gate's verdict can change under an open window
    // when the own endpoint's declaration is edited on the Advanced page.
    let refused = paragraph("");
    content.append(&refused);
    let learn = gtk::Button::with_label(l10n::writing_style_learn());
    learn.add_css_class("suggested-action");
    learn.set_halign(gtk::Align::Start);
    content.append(&learn);

    let library = adw::PreferencesGroup::new();
    let empty = named_row(l10n::writing_style_empty());
    library.add(&empty);
    content.append(&library);

    let page = Rc::new(Page {
        ctx: ctx.clone(),
        zone: ctx.app.timezone_settings().active,
        refused,
        learn,
        library,
        empty,
        rows: RefCell::new(Vec::new()),
        names: gtk::StringList::new(&[l10n::writing_style_none()]),
        ids: RefCell::new(Vec::new()),
        pickers: RefCell::new(Vec::new()),
        syncing: Cell::new(false),
        latest: RefCell::new(snapshot.clone()),
        sheet: RefCell::new(None),
    });
    content.append(&page.accounts_group(&snapshot.accounts));
    content.append(&credits_group(ctx));
    let learning = Rc::clone(&page);
    page.learn
        .connect_clicked(move |_| learning.start_learning());
    page.reconcile(&snapshot);
    let pushed = Rc::downgrade(&page);
    ctx.writing_style.register(move |snapshot| {
        if let Some(page) = pushed.upgrade() {
            page.reconcile(snapshot);
        }
    });
    content
}

/// The credits line, here and on the Allodia account's page. Built hidden when there is nothing to
/// state, so a balance the relay reports later can still appear.
pub(super) fn credits_group(ctx: &PageContext) -> adw::PreferencesGroup {
    let section = adw::PreferencesGroup::new();
    let row = plain_text_row();
    section.add(&row);
    let zone = ctx.app.timezone_settings().active;
    show_credits(&section, &row, &ctx.app.writing_styles(), &zone);
    let (shown, line) = (section.downgrade(), row.downgrade());
    ctx.writing_style.register(move |snapshot| {
        if let (Some(section), Some(row)) = (shown.upgrade(), line.upgrade()) {
            show_credits(&section, &row, snapshot, &zone);
        }
    });
    section
}

fn show_credits(
    section: &adw::PreferencesGroup,
    row: &adw::ActionRow,
    snapshot: &WritingStyleSnapshot,
    zone: &str,
) {
    let line = rules::shown_credits(snapshot)
        .map(|balance| rules::credits_line(balance, zone, l10n::active_locale()));
    row.set_title(line.as_deref().unwrap_or_default());
    section.set_visible(line.is_some());
}

/// The category's live widgets, and the bookkeeping that lets them be updated rather than replaced.
struct Page {
    ctx: PageContext,
    zone: String,
    refused: gtk::Label,
    learn: gtk::Button,
    library: adw::PreferencesGroup,
    /// The "no writing style yet" row, shown and hidden rather than added and removed.
    empty: adw::ActionRow,
    /// The rows on screen, by style id, so a snapshot can tell a new style from a renamed one.
    rows: RefCell<Vec<(String, adw::ActionRow)>>,
    /// **None** first, then the library; the model every account's picker draws from.
    names: gtk::StringList,
    /// The library's ids, parallel to `names` from index 1.
    ids: RefCell<Vec<String>>,
    /// Every account's picker, by account id.
    pickers: RefCell<Vec<(String, adw::ComboRow)>>,
    /// Set while the page is brought to a snapshot, so a picker's own handler does not store what
    /// the core just said as though the person had chosen it.
    syncing: Cell<bool>,
    /// The last snapshot, for the learning sheet: which route it names in its consent.
    latest: RefCell<WritingStyleSnapshot>,
    /// The learning sheet, while one is open.
    sheet: RefCell<Option<Rc<writing_style_learn::Sheet>>>,
}

impl Page {
    fn reconcile(self: &Rc<Self>, snapshot: &WritingStyleSnapshot) {
        let refusal = snapshot
            .refused
            .map(|refused| rules::refused_text(refused.mode));
        self.refused.set_text(refusal.unwrap_or_default());
        self.refused.set_visible(refusal.is_some());
        self.learn.set_visible(refusal.is_none());
        self.learn.set_sensitive(!snapshot.accounts.is_empty());
        // The whole of it guarded, names included: shortening the shared model moves a picker off
        // an entry that is going, and that move is not the person choosing either.
        self.syncing.set(true);
        self.sync_names(&snapshot.styles);
        self.sync_selections(&snapshot.accounts);
        self.syncing.set(false);
        self.sync_rows(&snapshot.styles);
        if let Some(sheet) = self.sheet.borrow().as_ref() {
            sheet.progress(snapshot.learning.as_ref());
        }
        self.latest.replace(snapshot.clone());
    }

    /// Brings the shared picker model to the library entry by entry, as Signatures does, so a
    /// picker's open popover is not rebuilt under the pointer.
    fn sync_names(&self, styles: &[WritingStyleRow]) {
        for (index, style) in styles.iter().enumerate() {
            let Ok(position) = u32::try_from(index + 1) else {
                break;
            };
            match self.names.string(position) {
                Some(current) if current == style.name => {}
                Some(_) => self.names.splice(position, 1, &[style.name.as_str()]),
                None => self.names.append(&style.name),
            }
        }
        let wanted = u32::try_from(styles.len())
            .unwrap_or(u32::MAX)
            .saturating_add(1);
        while self.names.n_items() > wanted {
            self.names.remove(self.names.n_items() - 1);
        }
        self.ids
            .replace(styles.iter().map(|style| style.id.clone()).collect());
    }

    fn sync_selections(&self, accounts: &[AccountWritingStyleRow]) {
        let ids = self.ids.borrow();
        for (account_id, picker) in self.pickers.borrow().iter() {
            let Some(account) = accounts
                .iter()
                .find(|account| &account.account_id == account_id)
            else {
                continue;
            };
            let selection = slot_selection(account.style.as_deref(), &ids);
            if picker.selected() != selection {
                picker.set_selected(selection);
            }
        }
    }

    /// Appends a new style, renames or re-describes one that changed, and removes a forgotten one.
    fn sync_rows(self: &Rc<Self>, styles: &[WritingStyleRow]) {
        let locale = l10n::active_locale();
        let mut rows = self.rows.borrow_mut();
        rows.retain(|(id, row)| {
            let kept = styles.iter().any(|style| &style.id == id);
            if !kept {
                self.library.remove(row);
            }
            kept
        });
        for style in styles {
            let summary = rules::style_summary(style, &self.zone, locale);
            if let Some((_, row)) = rows.iter().find(|(id, _)| id == &style.id) {
                if row.title() != style.name {
                    row.set_title(&style.name);
                }
                if row.subtitle().as_deref() != Some(summary.as_str()) {
                    row.set_subtitle(&summary);
                }
            } else {
                let row = self.style_row(style, &summary);
                self.library.add(&row);
                rows.push((style.id.clone(), row));
            }
        }
        self.empty.set_visible(styles.is_empty());
    }

    /// One style: its name, what it was learned from, and a way into it.
    fn style_row(self: &Rc<Self>, style: &WritingStyleRow, summary: &str) -> adw::ActionRow {
        let row = named_row(&style.name);
        row.set_subtitle(summary);
        row.set_activatable(true);
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        let page = Rc::downgrade(self);
        let id = style.id.clone();
        row.connect_activated(move |_| {
            if let Some(page) = page.upgrade() {
                page.open_style(&id);
            }
        });
        row
    }

    /// One picker per account, labelled by its address: the setting is per account, and a person
    /// who adds a second one later must not have to relearn that.
    fn accounts_group(
        self: &Rc<Self>,
        accounts: &[AccountWritingStyleRow],
    ) -> adw::PreferencesGroup {
        let section = group_titled(l10n::writing_style_accounts_heading());
        if accounts.is_empty() {
            section.add(&named_row(l10n::settings_accounts_empty()));
            return section;
        }
        for account in accounts {
            // A combo row carries its own title, for the reason `signatures::slot_picker` gives.
            // The model goes in before the handler: setting it selects the first entry, and that
            // is not the person choosing None.
            let picker = adw::ComboRow::new();
            picker.set_use_markup(false);
            picker.set_title(&account.email);
            picker.set_model(Some(&self.names));
            let page = Rc::downgrade(self);
            let account_id = account.account_id.clone();
            picker.connect_selected_notify(move |picker| {
                let Some(page) = page.upgrade() else {
                    return;
                };
                if page.syncing.get() {
                    return;
                }
                let chosen = slot_choice(picker.selected(), &page.ids.borrow());
                page.ctx
                    .app
                    .set_account_writing_style(account_id.clone(), chosen);
            });
            self.pickers
                .borrow_mut()
                .push((account.account_id.clone(), picker.clone()));
            section.add(&picker);
        }
        section
    }

    fn open_style(&self, id: &str) {
        if let Some(detail) = self.ctx.app.writing_style_detail(id.to_owned()) {
            writing_style_reveal::open(&self.ctx, &detail);
        }
    }

    fn start_learning(self: &Rc<Self>) {
        let page = Rc::downgrade(self);
        let sheet = writing_style_learn::open(&self.ctx, &self.latest.borrow(), move |style| {
            if let Some(page) = page.upgrade() {
                page.sheet.replace(None);
                page.open_style(&style);
            }
        });
        self.sheet.replace(Some(sheet));
    }
}

pub(super) fn paragraph(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_xalign(0.0);
    label
}
