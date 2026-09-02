//! Pure contact-editor state and intent construction: everything the GTK dialog next door
//! would otherwise decide inside a signal handler, where nothing can test it.
//!
//! The validation here is a **copy** of the core's, and deliberately so: the core refuses a
//! card with nothing to file it under, but it has no locale and cannot say which sentence to
//! put under the form. So the client decides what to say and the core stays the backstop,
//! exactly as the calendar editor does with an end before its start.

use mailcal_bindings::{ContactEdit, Intent};

/// A writable address book the create form can file a contact in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BookChoice {
    pub(crate) account: String,
    pub(crate) book: String,
    /// What the picker shows: the account's address, and the book's name beside it when the
    /// account has more than one.
    pub(crate) label: String,
    pub(crate) is_default: bool,
}

/// The card an editor was opened on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EditTarget {
    /// The person the row carried, so a card retired by a merge still resolves.
    pub(crate) person: String,
    pub(crate) account: String,
    pub(crate) card: String,
}

/// The values read out of the GTK form when the user saves.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ContactForm {
    pub(crate) given_name: String,
    pub(crate) surname: String,
    pub(crate) organization: String,
    pub(crate) title: String,
    pub(crate) emails: Vec<String>,
    pub(crate) phones: Vec<String>,
    pub(crate) book_index: u32,
}

/// Why a form cannot be saved, so the dialog can pick its sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FormError {
    /// Nothing to file the card under: no name, no organisation, no address.
    Empty,
    /// A value in the address list is not an address.
    Email,
}

/// An open create or edit form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContactEditor {
    /// The card being edited, or `None` for a create.
    pub(crate) editing: Option<EditTarget>,
    /// The values the form opens with.
    pub(crate) seed: ContactEdit,
    /// Where a create may file the contact. Empty on an edit, which files nowhere new.
    pub(crate) choices: Vec<BookChoice>,
    pub(crate) selected: u32,
}

impl ContactEditor {
    pub(crate) fn create(choices: Vec<BookChoice>) -> Self {
        let selected = choices
            .iter()
            .position(|choice| choice.is_default)
            .and_then(|index| u32::try_from(index).ok())
            .unwrap_or(0);
        Self {
            editing: None,
            seed: blank(),
            choices,
            selected,
        }
    }

    pub(crate) fn edit(target: EditTarget, seed: ContactEdit) -> Self {
        Self {
            editing: Some(target),
            seed,
            choices: Vec::new(),
            selected: 0,
        }
    }

    /// The intent a Save dispatches.
    ///
    /// # Errors
    ///
    /// Returns what is wrong with the form, so the dialog can say which.
    pub(crate) fn intent(&self, form: &ContactForm) -> Result<Intent, FormError> {
        let edit = trimmed(form);
        if edit.given_name.is_empty()
            && edit.surname.is_empty()
            && edit.organization.is_empty()
            && edit.emails.is_empty()
        {
            return Err(FormError::Empty);
        }
        if edit.emails.iter().any(|email| !is_address_shaped(email)) {
            return Err(FormError::Email);
        }
        if let Some(target) = &self.editing {
            return Ok(Intent::UpdateContact {
                person: target.person.clone(),
                account: target.account.clone(),
                card: target.card.clone(),
                edit,
            });
        }
        let choice = usize::try_from(form.book_index)
            .ok()
            .and_then(|index| self.choices.get(index));
        Ok(Intent::CreateContact {
            account: choice.map(|choice| choice.account.clone()),
            address_book: choice.map(|choice| choice.book.clone()),
            edit,
        })
    }
}

/// An empty form.
fn blank() -> ContactEdit {
    ContactEdit {
        given_name: String::new(),
        surname: String::new(),
        organization: String::new(),
        title: String::new(),
        emails: Vec::new(),
        phones: Vec::new(),
    }
}

/// The form with every value trimmed and its blank rows dropped.
///
/// The core trims too; doing it here as well is what makes the validation above agree with
/// the refusal below it. A form holding one empty address row is a form with no addresses,
/// and telling the user otherwise would be a message about a row they can see is blank.
fn trimmed(form: &ContactForm) -> ContactEdit {
    let list = |values: &[String]| {
        values
            .iter()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .collect()
    };
    ContactEdit {
        given_name: form.given_name.trim().to_owned(),
        surname: form.surname.trim().to_owned(),
        organization: form.organization.trim().to_owned(),
        title: form.title.trim().to_owned(),
        emails: list(&form.emails),
        phones: list(&form.phones),
    }
}

/// Whether a string is shaped like an email address; the same test the core applies.
fn is_address_shaped(value: &str) -> bool {
    value.split_once('@').is_some_and(|(local, domain)| {
        !local.is_empty() && !domain.is_empty() && !domain.contains('@') && !domain.starts_with('.')
    })
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
