//! The signature library: the user's named, reusable signatures and where they are stored.
//!
//! Signatures are **standalone entities**, not a per-account string: one is authored once and
//! reused across every account, which is why the library is its own store rather than a field on
//! an account. Which signature an account uses (for new messages, and for replies/forwards) is a
//! small per-account assignment that lives with the other preferences
//! ([`crate::Preferences::signature_assignments`]). See `docs/signatures.md`.
//!
//! **Why a separate file from `preferences.toml`.** A signature body carries its images inline as
//! base64 `data:` URIs, so it is the largest thing the app persists. Every preference write is a
//! read-modify-write of the whole file, so keeping signatures out of `preferences.toml` means
//! toggling a swipe action does not rewrite a logo's bytes.
//!
//! **Privacy.** A signature body is user content: a name, a title, a phone number, a logo. It is
//! never logged (`docs/logging.md`), which is why [`StoredSignature`]'s `Debug` prints lengths
//! rather than text.

use std::{
    collections::BTreeMap,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// The stable identity of one signature in the library.
///
/// Opaque and random (minted by the product core), never derived from the name; renaming a
/// signature must not break the accounts that point at it. A newtype rather than a bare `String`
/// so it cannot be passed where an account id belongs, which is a live risk: the assignment API
/// takes both at once.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SignatureId(String);

impl SignatureId {
    /// Creates a signature id when the value is non-empty after trimming and carries no control
    /// characters. The store is a plain TOML file a user can hand-edit, so a value read back from
    /// it is validated rather than trusted.
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        if value.trim().is_empty() || value.chars().any(char::is_control) {
            None
        } else {
            Some(Self(value))
        }
    }

    /// The id as stored.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SignatureId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SignatureId").field(&self.0).finish()
    }
}

/// One authored signature: the name the user picks it by, plus the two body renderings.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredSignature {
    /// The user's name for it ("Work", "Personal"). Display-only; accounts point at the id.
    pub name: String,
    /// The signature's HTML, self-contained: any images are inline `data:` URIs, so the library
    /// is one file with no side-car blobs to lose.
    pub body_html: String,
    /// The plain-text rendering, for an outgoing message's `text/plain` part.
    #[serde(default)]
    pub body_plain: String,
}

impl fmt::Debug for StoredSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Lengths, never content: a signature body is user content and must not reach a log.
        f.debug_struct("StoredSignature")
            .field("name_len", &self.name.len())
            .field("body_html_len", &self.body_html.len())
            .field("body_plain_len", &self.body_plain.len())
            .finish()
    }
}

/// The persisted signature library: the signatures themselves, plus the order the user arranged
/// them in.
///
/// The order is stored separately because [`BTreeMap`] sorts by the (random) id, which is no
/// order at all to a reader. The map keeps the serialized TOML stable across writes; `order` is
/// what a list renders.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signatures {
    /// Every signature, keyed by id.
    #[serde(default)]
    pub entries: BTreeMap<SignatureId, StoredSignature>,
    /// The display order. Ids here that are absent from `entries` are ignored, and entries
    /// missing from it still appear (see [`Signatures::ordered`]): a hand-edited file must not
    /// be able to hide a signature the user can no longer delete.
    #[serde(default)]
    pub order: Vec<SignatureId>,
}

impl Signatures {
    /// The signatures in display order: those named in `order` first, then any entry `order`
    /// forgot (appended by id order, so the result is deterministic).
    #[must_use]
    pub fn ordered(&self) -> Vec<(&SignatureId, &StoredSignature)> {
        let mut listed: Vec<(&SignatureId, &StoredSignature)> =
            Vec::with_capacity(self.entries.len());
        for id in &self.order {
            if let Some((id, signature)) = self.entries.get_key_value(id) {
                listed.push((id, signature));
            }
        }
        for (id, signature) in &self.entries {
            if !self.order.contains(id) {
                listed.push((id, signature));
            }
        }
        listed
    }

    /// One signature by id.
    #[must_use]
    pub fn get(&self, id: &SignatureId) -> Option<&StoredSignature> {
        self.entries.get(id)
    }

    /// Adds a signature at the end of the display order. A repeated id replaces the entry in
    /// place and leaves the order alone.
    pub fn insert(&mut self, id: SignatureId, signature: StoredSignature) {
        if !self.order.contains(&id) {
            self.order.push(id.clone());
        }
        self.entries.insert(id, signature);
    }

    /// Removes a signature and its place in the order. Returns whether it existed.
    pub fn remove(&mut self, id: &SignatureId) -> bool {
        self.order.retain(|entry| entry != id);
        self.entries.remove(id).is_some()
    }
}

/// Which of an account's two signature slots is meant. Reply and forward share one slot
/// deliberately (Outlook's grouping, and what people expect): a reply and a forward are both
/// "continuing an existing message", and splitting them produces a setting nobody sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureSlot {
    /// The signature a brand-new message opens with.
    NewMessage,
    /// The signature a reply or a forward opens with.
    ReplyForward,
}

/// Which signature an account uses in each slot. `None` in a slot means **no signature** there;
/// there is no separate "signatures on" flag, because "None for both" already says it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSignatureAssignment {
    /// The signature for a new message, or `None` for no signature.
    #[serde(default)]
    pub new_message: Option<SignatureId>,
    /// The signature for a reply or a forward, or `None` for no signature.
    #[serde(default)]
    pub reply_forward: Option<SignatureId>,
}

impl AccountSignatureAssignment {
    /// The signature assigned to one slot.
    #[must_use]
    pub fn slot(&self, slot: SignatureSlot) -> Option<&SignatureId> {
        match slot {
            SignatureSlot::NewMessage => self.new_message.as_ref(),
            SignatureSlot::ReplyForward => self.reply_forward.as_ref(),
        }
    }

    /// Assigns (or clears, with `None`) one slot.
    pub fn set_slot(&mut self, slot: SignatureSlot, signature: Option<SignatureId>) {
        match slot {
            SignatureSlot::NewMessage => self.new_message = signature,
            SignatureSlot::ReplyForward => self.reply_forward = signature,
        }
    }

    /// Whether both slots are empty: the state an account with nothing assigned is left in, and
    /// the cue to drop its entry rather than persist an empty table.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.new_message.is_none() && self.reply_forward.is_none()
    }
}

/// The library file's name, next to `preferences.toml` in the app data directory.
const FILE_NAME: &str = "signatures.toml";

/// The signature library's path inside the app data directory `base`.
#[must_use]
pub fn signatures_path(base: impl AsRef<Path>) -> PathBuf {
    base.as_ref().join(FILE_NAME)
}

/// Loads the signature library from `path`. A missing or unreadable/unparseable file yields an
/// empty library rather than an error, like [`load_preferences`](crate::load_preferences), this
/// is best-effort local state, and a host that cannot read it simply shows no signatures.
#[must_use]
pub fn load_signatures(path: impl AsRef<Path>) -> Signatures {
    fs::read_to_string(path)
        .ok()
        .and_then(|body| toml::from_str(&body).ok())
        .unwrap_or_default()
}

/// Writes the signature library to `path` as TOML, creating parent directories as needed.
///
/// # Errors
///
/// Returns an [`io::Error`] if the parent directory or file cannot be written (a TOML
/// serialization failure is mapped to [`io::ErrorKind::InvalidData`]).
pub fn save_signatures(path: impl AsRef<Path>, signatures: &Signatures) -> io::Result<()> {
    let body = toml::to_string(signatures)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, body)
}

#[cfg(test)]
mod tests {
    use super::{
        AccountSignatureAssignment, SignatureId, SignatureSlot, Signatures, StoredSignature,
        load_signatures, save_signatures, signatures_path,
    };

    fn id(value: &str) -> SignatureId {
        SignatureId::new(value).expect("valid id")
    }

    fn signature(name: &str) -> StoredSignature {
        StoredSignature {
            name: name.to_owned(),
            body_html: format!("<p>{name}</p>"),
            body_plain: name.to_owned(),
        }
    }

    #[test]
    fn a_library_round_trips_through_toml_with_its_order_preserved() {
        let dir = std::env::temp_dir().join("mailcal-signatures-roundtrip-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = signatures_path(&dir);

        let mut library = Signatures::default();
        // Inserted in an order the id sort would NOT produce, so the assertion below proves the
        // stored `order` is what a list renders: not the BTreeMap's key order.
        library.insert(id("zzz"), signature("Work"));
        library.insert(id("aaa"), signature("Personal"));
        save_signatures(&path, &library).unwrap();

        let reloaded = load_signatures(&path);
        let names: Vec<&str> = reloaded
            .ordered()
            .iter()
            .map(|(_, entry)| entry.name.as_str())
            .collect();
        assert_eq!(names, ["Work", "Personal"]);
        assert_eq!(
            reloaded.get(&id("aaa")).unwrap().body_html,
            "<p>Personal</p>"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_entry_missing_from_the_order_still_lists() {
        // A hand-edited (or half-written) file must never be able to hide a signature: an entry
        // the order forgot still shows, so the user can see and delete it.
        let mut library = Signatures::default();
        library.insert(id("one"), signature("One"));
        library.entries.insert(id("orphan"), signature("Orphan"));

        let names: Vec<&str> = library
            .ordered()
            .iter()
            .map(|(_, entry)| entry.name.as_str())
            .collect();
        assert_eq!(names, ["One", "Orphan"]);
    }

    #[test]
    fn removing_a_signature_drops_it_from_the_order_too() {
        let mut library = Signatures::default();
        library.insert(id("one"), signature("One"));
        library.insert(id("two"), signature("Two"));

        assert!(library.remove(&id("one")));
        assert!(!library.remove(&id("one")));
        assert_eq!(library.order, vec![id("two")]);
    }

    #[test]
    fn a_signature_body_never_reaches_its_debug_output() {
        // The privacy rule made checkable: a signature carries a name, a phone number, a logo.
        // `{:?}` is what a stray log line reaches for, so it must print lengths only.
        let rendered = format!("{:?}", signature("Work"));
        assert!(!rendered.contains("Work"));
        assert!(rendered.contains("body_html_len"));
    }

    #[test]
    fn an_assignment_addresses_its_two_slots_independently() {
        let mut assignment = AccountSignatureAssignment::default();
        assert!(assignment.is_empty());

        assignment.set_slot(SignatureSlot::NewMessage, Some(id("one")));
        assert_eq!(assignment.slot(SignatureSlot::NewMessage), Some(&id("one")));
        assert_eq!(assignment.slot(SignatureSlot::ReplyForward), None);
        assert!(!assignment.is_empty());

        assignment.set_slot(SignatureSlot::NewMessage, None);
        assert!(assignment.is_empty());
    }

    #[test]
    fn a_blank_or_control_bearing_id_is_refused() {
        // The store is hand-editable TOML; a control character in a key would break the file it
        // is written back into, so it is rejected at the boundary rather than round-tripped.
        assert!(SignatureId::new("  ").is_none());
        assert!(SignatureId::new("a\nb").is_none());
        assert!(SignatureId::new("kK3-x_9").is_some());
    }
}
