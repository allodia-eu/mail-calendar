//! A form pane that stays on screen while its connect runs, and while its answer comes back.
//!
//! The pane the person typed into is the only place their fields live, the secret above all, so
//! a connect result is drawn *into* it rather than by building a second pane from the stored
//! form: what is stored was never the secret, and a rebuild would ask for the whole server
//! again in order to answer a question about the server they just gave
//! (`docs/certificate-exceptions.md` rule 6). The other clients keep the form mounted for the
//! same reason; on Apple the connect replaces the button and nothing else.
//!
//! So two things outlive any one result and are held here rather than captured when the pane is
//! built: the gate that decides whether Connect can act, and the certificate a refusal carried,
//! which the next submission has to send back as the acceptance.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::RejectedCertificate;

use super::setup_widgets::{ConnectGate, certificate_gate, show_error};

/// The widget name of a step's Back button, by which the window hands it to the step's pane.
pub(super) const BACK: &str = "setup-back";

/// The first widget under `root` named `name`.
pub(super) fn named(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if root.widget_name() == name {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(found) = named(&widget, name) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

/// The readout that stands where Connect was while a connect runs: small and in the row, not
/// the page-sized spinner a whole phase gets, because the form it belongs to is still on screen.
pub(super) fn connecting_readout(message: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let spinner = gtk::Spinner::new();
    spinner.set_spinning(true);
    row.append(&spinner);
    row.append(&super::setup_widgets::body(message));
    row.set_visible(false);
    row
}

/// The parts of a mounted pane a connect result changes.
#[derive(Debug, Clone)]
pub(super) struct ConnectPane {
    /// Where a refusal draws itself: emptied and refilled per result, so nothing above it moves.
    feedback: gtk::Box,
    gate: ConnectGate,
    connect: gtk::Button,
    progress: gtk::Widget,
    /// What the last refusal carried, read at click time because the pane outlives it.
    refused: Rc<RefCell<Option<RejectedCertificate>>>,
    /// Whether this route can act on a refused certificate at all. A JMAP account's stored
    /// config carries no exception, so its refusal is reported as plain text
    /// (`docs/certificate-exceptions.md` rule 8).
    offers_exception: bool,
    /// The step's Back, which waits while a connect runs; handed over by [`Self::hold_back`].
    back: Rc<RefCell<Option<gtk::Widget>>>,
}

impl ConnectPane {
    pub(super) fn new(
        feedback: &gtk::Box,
        gate: ConnectGate,
        connect: &gtk::Button,
        progress: &impl IsA<gtk::Widget>,
        offers_exception: bool,
    ) -> Self {
        let pane = Self {
            feedback: feedback.clone(),
            gate,
            connect: connect.clone(),
            progress: progress.as_ref().clone(),
            refused: Rc::new(RefCell::new(None)),
            back: Rc::new(RefCell::new(None)),
            offers_exception,
        };
        pane.set_connecting(false);
        pane
    }

    /// Hands over the step's Back button, found in `content` by its name, so a running connect
    /// can hold it.
    pub(super) fn hold_back(&self, content: &impl IsA<gtk::Widget>) {
        self.back.replace(named(content.as_ref(), BACK));
    }

    /// The certificate to send back with the next submission: what the person accepted on this
    /// pane, if anything. Cloned into the submit path, which cannot borrow across the click.
    pub(super) fn accepted(&self) -> Rc<RefCell<Option<RejectedCertificate>>> {
        Rc::clone(&self.refused)
    }

    /// Draws what the connect answered. The panel replaces the transport's own message rather
    /// than sitting under it, so the two never state the refusal twice
    /// (`docs/certificate-exceptions.md` rule 7).
    pub(super) fn show_result(
        &self,
        error: Option<&str>,
        certificate: Option<&RejectedCertificate>,
    ) {
        while let Some(child) = self.feedback.first_child() {
            self.feedback.remove(&child);
        }
        let offered = certificate.filter(|_| self.offers_exception);
        self.refused.replace(offered.cloned());
        let accepted = certificate_gate(&self.feedback, offered);
        self.gate.offers(accepted.as_ref());
        show_error(&self.feedback, error.filter(|_| offered.is_none()));
    }

    /// A connect is running: the button becomes the progress readout in its place, so the form
    /// stays legible and nothing below it jumps. Back waits for it too, because its answer belongs
    /// to the step on screen (`docs/account-autodetect.md` rule 12).
    pub(super) fn set_connecting(&self, connecting: bool) {
        self.connect.set_visible(!connecting);
        self.progress.set_visible(connecting);
        if let Some(back) = self.back.borrow().as_ref() {
            back.set_sensitive(!connecting);
        }
        if connecting {
            while let Some(child) = self.feedback.first_child() {
                self.feedback.remove(&child);
            }
        }
    }
}
