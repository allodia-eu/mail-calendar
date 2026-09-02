//! Pure contacts state for the Linux client: the people on screen, the search that narrowed them,
//! and the person opened beside them.
//!
//! Everything that could differ between clients arrives already decided; the join, the ordering,
//! the section letter, the monogram and the matching all live in the core (`docs/contacts.md`
//! §2), and this file adds no sorting, no re-bucketing and no matching of its own. What it does
//! is turn one flat ordered list into what a row draws: the section letter only where it changes,
//! the "In N accounts" disclosure only where there is a merge to disclose, and the localised copy
//! the core deliberately does not carry.

use mailcal_bindings::{
    AccountRow, ContactDetail, ContactEdit, ContactRow, ContactTarget, ContactValue,
    ContactWriteStatus, MailcalApp,
};

use super::editor::{BookChoice, ContactEditor, EditTarget};
use crate::{l10n, ui::avatar::AvatarData};

/// What the list column has to show. The two empty states are deliberately different sentences:
/// telling someone who has just searched "No contacts yet" reads as though theirs had vanished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ListState {
    /// People to render.
    Rows,
    /// Nothing has synced yet, and no search is narrowing it.
    NoContacts,
    /// A search is active and matched nobody.
    NoResults,
}

/// One list row as the pane draws it; the row is rebuilt when this changes, so a field the row
/// shows and this omits would leave a stale row on screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PersonRow {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) email: String,
    pub(crate) avatar: AvatarData,
    /// The A–Z header to draw **above** this row, or `None` when the row before it already
    /// carries it.
    pub(crate) section: Option<String>,
    /// The merge disclosure, or `None` for an ordinary contact.
    pub(crate) accounts: Option<String>,
}

/// One person's detail: every value, grouped, with the accounts that supplied it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PersonDetail {
    /// The person's id, which an edit carries so a card retired by a merge still resolves.
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) avatar: AvatarData,
    pub(crate) groups: Vec<ValueGroup>,
    /// The accounts named under "Also in": empty unless this person really is a merge.
    pub(crate) accounts: Vec<String>,
    /// The cards this person can be edited through. Empty means every source is read-only,
    /// and the detail says so instead of offering an edit that would fail on press.
    pub(crate) editable: Vec<CardChoice>,
}

/// One card an edit could go to, labelled by the account the user knows it by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CardChoice {
    pub(crate) account: String,
    pub(crate) card: String,
    pub(crate) label: String,
}

/// One headed group of values (the emails, the phone numbers, …).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ValueGroup {
    pub(crate) heading: &'static str,
    pub(crate) values: Vec<ValueRow>,
}

/// One value and where it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ValueRow {
    pub(crate) value: String,
    /// The accounts carrying it; empty for a person who is in only one, where naming it
    /// disambiguates nothing.
    pub(crate) accounts: String,
}

/// The contacts surface's whole state.
#[derive(Default)]
pub(crate) struct ContactsModel {
    rows: Vec<PersonRow>,
    query: String,
    opened: Option<PersonDetail>,
    lookup: u64,
    /// Where a new contact could be filed. Empty means nowhere, and the surface offers no
    /// create at all rather than one that cannot succeed.
    targets: Vec<BookChoice>,
    /// The open editor, and a counter the pane compares against so one editor is presented
    /// once. The same shape the calendar's dialog uses, for the same reason: `render` runs on
    /// every model change, and a dialog keyed on `Some`/`None` alone would be re-presented by
    /// an unrelated one.
    editor: Option<ContactEditor>,
    editor_generation: u64,
    /// The "which account?" question, when a merged person has more than one editable card.
    card_choice: Option<Vec<CardChoice>>,
    choice_generation: u64,
    write_status: ContactWriteStatus,
}

impl ContactsModel {
    pub(crate) fn rows(&self) -> &[PersonRow] {
        &self.rows
    }

    pub(crate) fn opened(&self) -> Option<&PersonDetail> {
        self.opened.as_ref()
    }

    pub(crate) fn query(&self) -> &str {
        &self.query
    }

    /// Whether the surface may offer to create a contact: only with somewhere to put one.
    pub(crate) fn can_create(&self) -> bool {
        !self.targets.is_empty()
    }

    pub(crate) fn write_status(&self) -> ContactWriteStatus {
        self.write_status
    }

    pub(crate) fn editor(&self) -> Option<&ContactEditor> {
        self.editor.as_ref()
    }

    pub(crate) fn editor_generation(&self) -> u64 {
        self.editor_generation
    }

    pub(crate) fn card_choice(&self) -> Option<&[CardChoice]> {
        self.card_choice.as_deref()
    }

    pub(crate) fn choice_generation(&self) -> u64 {
        self.choice_generation
    }

    pub(crate) fn state(&self) -> ListState {
        if !self.rows.is_empty() {
            return ListState::Rows;
        }
        if self.query.trim().is_empty() {
            ListState::NoContacts
        } else {
            ListState::NoResults
        }
    }

    /// Reads the snapshot the core just published.
    pub(crate) fn refresh(&mut self, app: &MailcalApp) {
        self.rows = people(&app.contact_list().rows);
    }

    /// Reads the write status the core just published.
    pub(crate) fn refresh_write_status(&mut self, app: &MailcalApp) {
        self.write_status = app.contact_write_status();
    }

    /// Files the writable address books, read off the UI thread when the surface opened.
    pub(crate) fn set_targets(&mut self, targets: &[ContactTarget], accounts: &[AccountRow]) {
        // The account's address is what a user recognises; the book's name only matters when
        // one account offers several, where the address alone would repeat.
        let mut per_account = std::collections::BTreeMap::new();
        for target in targets {
            *per_account.entry(target.account.clone()).or_insert(0_usize) += 1;
        }
        self.targets = targets
            .iter()
            .map(|target| {
                let account = account_label(&target.account, accounts);
                let label = if per_account.get(&target.account).copied().unwrap_or(0) > 1
                    && !target.name.is_empty()
                {
                    format!("{account} ({})", target.name)
                } else {
                    account
                };
                BookChoice {
                    account: target.account.clone(),
                    book: target.address_book.clone(),
                    label,
                    is_default: target.is_default,
                }
            })
            .collect();
    }

    /// Opens the create form. The caller has already checked there is somewhere to file it.
    pub(crate) fn begin_create(&mut self) {
        self.open_editor(ContactEditor::create(self.targets.clone()));
    }

    /// Opens the edit form on one card, seeded with what that card holds.
    pub(crate) fn begin_edit(&mut self, target: EditTarget, seed: ContactEdit) {
        self.open_editor(ContactEditor::edit(target, seed));
    }

    /// Raises the "which account?" question for a person filed in several.
    pub(crate) fn ask_which_card(&mut self, choices: Vec<CardChoice>) {
        self.card_choice = Some(choices);
        self.choice_generation = self.choice_generation.wrapping_add(1);
    }

    /// The person the detail is showing, and the cards it can be edited through.
    pub(crate) fn open_person(&self) -> Option<(&str, &[CardChoice])> {
        self.opened
            .as_ref()
            .map(|person| (person.id.as_str(), person.editable.as_slice()))
    }

    pub(crate) fn close_editor(&mut self) {
        self.editor = None;
        self.card_choice = None;
    }

    fn open_editor(&mut self, editor: ContactEditor) {
        self.card_choice = None;
        self.editor = Some(editor);
        self.editor_generation = self.editor_generation.wrapping_add(1);
    }

    /// Entering the surface. The query is dropped here **and** dispatched as an empty
    /// `SearchContacts`, as one action: the search box is view state that dies with the view, but
    /// the query lives in the core, so leaving mid-search and coming back would otherwise show a
    /// filtered list under an empty box; a narrowing the user can no longer see.
    pub(crate) fn entered(&mut self) {
        self.query.clear();
    }

    pub(crate) fn set_query(&mut self, query: String) {
        self.query = query;
    }

    /// Claims the next detail lookup, invalidating any still in flight.
    ///
    /// `contact_detail` is network-free but blocks on the core's runtime and lands on the store's
    /// connection thread, so it runs off the UI thread; and two people opened in quick
    /// succession answer in whatever order that thread hands them back. The later **request**
    /// wins, never the earlier answer.
    pub(crate) fn begin_lookup(&mut self) -> u64 {
        self.lookup = self.lookup.wrapping_add(1);
        self.lookup
    }

    /// Files a lookup's answer, unless a newer one has already been asked for.
    ///
    /// `None` means the person is genuinely gone: never merely renumbered. Merging retires ids
    /// and the core keeps the retired ones pointing at the survivor, so a row still held after a
    /// background sync merged it opens fine without refreshing the list first.
    pub(crate) fn finish_lookup(
        &mut self,
        lookup: u64,
        detail: Option<&ContactDetail>,
        accounts: &[AccountRow],
    ) {
        if lookup != self.lookup {
            return;
        }
        self.opened = detail.map(|detail| build_detail(detail, accounts));
    }
}

/// Projects the core's ordered rows, deciding each row's header from the one before it.
///
/// Compared with the previous row rather than re-bucketed by key: the core hands back one flat
/// ordered list, and re-grouping it here would be a second ordering that could disagree with the
/// first. Order in, order out.
fn people(rows: &[ContactRow]) -> Vec<PersonRow> {
    let mut previous: Option<&str> = None;
    let mut projected = Vec::with_capacity(rows.len());
    for row in rows {
        let section = (previous != Some(row.section.as_str())).then(|| row.section.clone());
        previous = Some(&row.section);
        projected.push(PersonRow {
            id: row.id.clone(),
            name: display_name(&row.display_name),
            email: row.primary_email.clone(),
            avatar: AvatarData::from(&row.avatar),
            section,
            accounts: discloses_accounts(row.account_count)
                .then(|| l10n::contacts_in_accounts(i64::from(row.account_count))),
        });
    }
    projected
}

/// Whether a row must say it is a merge.
///
/// Only above one: "In 1 accounts" on every ordinary contact is noise, and ungrammatical noise.
/// The count is of distinct **accounts**, which the core has already collapsed to; a client must
/// not recount from the source cards.
fn discloses_accounts(count: u32) -> bool {
    count > 1
}

/// The name a row shows.
///
/// A card may legitimately carry an address and no name, and the core emits an **empty** name for
/// it rather than a placeholder of its own; one in the core could only ever be English, and a
/// client cannot substitute for a string it has no way to detect. Empty is the signal.
fn display_name(name: &str) -> String {
    if name.is_empty() {
        l10n::contacts_no_name().to_owned()
    } else {
        name.to_owned()
    }
}

fn build_detail(detail: &ContactDetail, accounts: &[AccountRow]) -> PersonDetail {
    let spans = detail.accounts.len() > 1;
    let mut groups = Vec::new();
    for (heading, values) in [
        (l10n::contacts_section_emails(), &detail.emails),
        (l10n::contacts_section_phones(), &detail.phones),
        (
            l10n::contacts_section_organizations(),
            &detail.organizations,
        ),
        (l10n::contacts_section_titles(), &detail.titles),
    ] {
        if let Some(group) = value_group(heading, values, accounts, spans) {
            groups.push(group);
        }
    }
    PersonDetail {
        id: detail.id.clone(),
        editable: detail
            .editable_cards
            .iter()
            .map(|card| CardChoice {
                account: card.account.clone(),
                card: card.card.clone(),
                label: account_label(&card.account, accounts),
            })
            .collect(),
        name: display_name(&detail.display_name),
        avatar: AvatarData::from(&detail.avatar),
        groups,
        // "Also in" is the *explanation* of the list row's "In N accounts", so it exists exactly
        // where that disclosure does and nowhere else.
        accounts: if spans {
            detail
                .accounts
                .iter()
                .map(|id| account_label(id, accounts))
                .collect()
        } else {
            Vec::new()
        },
    }
}

fn value_group(
    heading: &'static str,
    values: &[ContactValue],
    accounts: &[AccountRow],
    spans: bool,
) -> Option<ValueGroup> {
    if values.is_empty() {
        return None;
    }
    Some(ValueGroup {
        heading,
        values: values
            .iter()
            .map(|value| ValueRow {
                value: value.value.clone(),
                accounts: if spans {
                    value
                        .accounts
                        .iter()
                        .map(|id| account_label(id, accounts))
                        .collect::<Vec<_>>()
                        .join(", ")
                } else {
                    String::new()
                },
            })
            .collect(),
    })
}

/// The address the user knows an account by.
///
/// The core's ids are internal (`eva@example.test@jmap:127.0.0.1:12080`); showing one is both ugly
/// and a leak of how ids are built. An id whose account has since been removed falls back to
/// itself rather than vanishing; a value with no visible source is worse than an ugly one.
fn account_label(id: &str, accounts: &[AccountRow]) -> String {
    accounts
        .iter()
        .find(|account| account.id == id)
        .map_or_else(|| id.to_owned(), |account| account.email.clone())
}

#[cfg(test)]
impl ContactsModel {
    /// A model already in a given state, for the widget tests next door; they drive the real
    /// `render`, so they need a model without a core behind it.
    pub(super) fn fixture(
        rows: &[ContactRow],
        query: &str,
        opened: Option<&ContactDetail>,
        accounts: &[AccountRow],
    ) -> Self {
        let mut model = Self {
            rows: people(rows),
            query: query.to_owned(),
            ..Self::default()
        };
        let lookup = model.begin_lookup();
        model.finish_lookup(lookup, opened, accounts);
        model
    }

    /// The same fixture, with somewhere a contact could be saved.
    pub(super) fn with_targets(
        mut self,
        targets: &[ContactTarget],
        accounts: &[AccountRow],
    ) -> Self {
        self.set_targets(targets, accounts);
        self
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
