// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: LicenseRef-Allodia-1.0

//! The four calls every synced collection shares, parameterised by where it lives.
//!
//! A collection is a set of records the service keeps per person and hands to each of their
//! devices: the mail-account list and the writing styles. The rules are the same for both, and
//! [`crate::AccountService::list_accounts`] and its neighbours state them: an opaque id minted on
//! the first store, a version every write names, an idempotency key on the create, and a
//! tombstone for a removal. What differs is the path, the key a payload travels under, and the
//! payload, which is what [`SyncedRecord`] names.

use std::{fmt, fmt::Write as _, marker::PhantomData};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    API_BASE_PATH, AccountService, Error, Method, Refusal, Request, Transport,
    accounts::SyncedAccount, reconcile::Fingerprint, styles::StyleRecord,
};

/// A record the service keeps per person and syncs between their devices.
pub trait SyncedRecord: Clone + DeserializeOwned {
    /// Where the collection lives, below the API's base path.
    const PATH: &'static str;
    /// The key the payload travels under in a write.
    const PAYLOAD: &'static str;
    /// What travels.
    type Payload: Fingerprint + PartialEq;
    /// What a read answers with.
    type List: DeserializeOwned;

    /// The service's id for the record.
    fn id(&self) -> &str;
    /// The version it is at.
    fn version(&self) -> u64;
    /// What it holds.
    fn payload(&self) -> &Self::Payload;
    /// This collection's record, when a conflict carries one.
    fn in_conflict(with: &ConflictWith) -> Option<&Self>;
}

/// A record the person removed on some device.
///
/// It comes back as an id rather than as its content, so each device decides what a removal means
/// for it: a mailbox the person may still want locally is a question for them, and a writing style
/// is simply forgotten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tombstone {
    /// Which record went.
    pub id: String,
    /// The version the deletion itself wrote.
    pub version: u64,
    /// When it went (RFC 3339).
    pub deleted_at: String,
}

/// What the server holds instead of what the caller expected.
///
/// A record can be gone as well as moved, and replaying a create whose record was since deleted
/// answers with the tombstone rather than storing it again, which would be the resurrection bug
/// wearing a different hat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConflictWith {
    /// An account, at the version the server holds.
    Record(Box<SyncedAccount>),
    /// A tombstone: the record is gone, and this is when.
    Tombstone(Box<Tombstone>),
    /// A writing style, at the version the server holds.
    Style(Box<StyleRecord>),
}

/// The `409` body: what the server holds now.
#[derive(Deserialize)]
struct ConflictBody {
    data: ConflictData,
}

#[derive(Deserialize)]
struct ConflictData {
    current: Option<ConflictWith>,
}

/// The four calls, against one collection.
pub struct SyncedCollection<'a, R> {
    service: &'a AccountService,
    record: PhantomData<fn() -> R>,
}

impl<R: SyncedRecord> fmt::Debug for SyncedCollection<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncedCollection")
            .field("path", &R::PATH)
            .finish_non_exhaustive()
    }
}

impl<'a, R: SyncedRecord> SyncedCollection<'a, R> {
    /// The collection `R` names, at `service`.
    #[must_use]
    pub fn new(service: &'a AccountService) -> Self {
        Self {
            service,
            record: PhantomData,
        }
    }

    fn url(&self) -> String {
        format!("{}{API_BASE_PATH}/{}", self.service.base_url(), R::PATH)
    }

    /// Everything this person syncs, or (with `since`) what changed after it.
    ///
    /// # Errors
    /// [`Error::Unauthorized`] when the token needs refreshing; [`Error::Transport`] when the
    /// request never arrived; [`Error::Malformed`] when the answer cannot be read.
    pub fn list(
        &self,
        transport: &dyn Transport,
        access_token: &str,
        since: Option<&str>,
    ) -> Result<R::List, Error> {
        let mut url = self.url();
        if let Some(since) = since {
            url.push_str("?since=");
            url.push_str(&encode_query(since));
        }
        let body = send(transport, &Request::get(url, access_token))?;
        parse(&body)
    }

    /// Store a record the service has never seen, and learn the id it minted.
    ///
    /// `idempotency_key` is the caller's own, held across retries of **this** create.
    ///
    /// # Errors
    /// [`Error::Refused`] when the service declined it with a code; otherwise as
    /// [`SyncedCollection::list`].
    pub fn create(
        &self,
        transport: &dyn Transport,
        access_token: &str,
        payload: &R::Payload,
        idempotency_key: &str,
    ) -> Result<R, Error> {
        let body = send(
            transport,
            &Request {
                url: self.url(),
                bearer: access_token.to_owned(),
                method: Method::Post,
                body: Some(wrap::<R>(None, payload)?),
                idempotency_key: Some(idempotency_key.to_owned()),
            },
        )?;
        parse(&body)
    }

    /// Replace a record the caller can already name, at the version it last read.
    ///
    /// # Errors
    /// [`Error::Conflict`] when `version` is not the one the server holds, carrying what it holds
    /// instead; otherwise as [`SyncedCollection::create`].
    pub fn update(
        &self,
        transport: &dyn Transport,
        access_token: &str,
        id: &str,
        version: u64,
        payload: &R::Payload,
    ) -> Result<R, Error> {
        let sent = send(
            transport,
            &Request {
                url: format!("{}/{id}", self.url()),
                bearer: access_token.to_owned(),
                method: Method::Put,
                body: Some(wrap::<R>(Some(version), payload)?),
                idempotency_key: None,
            },
        );
        match sent {
            Ok(body) => parse(&body),
            // A write whose response was lost re-sends the same base version and is refused, and
            // what the refusal carries is the write it was making. That is not a disagreement to
            // put in front of anybody: what the caller wanted stored is stored, and the only thing
            // missing was the receipt.
            Err(Error::Conflict(Some(with))) => match R::in_conflict(&with) {
                Some(current) if current.payload() == payload => Ok(current.clone()),
                _ => Err(Error::Conflict(Some(with))),
            },
            Err(other) => Err(other),
        }
    }

    /// Mark a record removed, so the person's other devices learn it went.
    ///
    /// Already gone is success: the caller wanted the record gone and it is gone.
    ///
    /// # Errors
    /// [`Error::Conflict`] when `version` is not the one the server holds: the person removed
    /// something that has since moved. Otherwise as [`SyncedCollection::list`].
    pub fn delete(
        &self,
        transport: &dyn Transport,
        access_token: &str,
        id: &str,
        version: u64,
    ) -> Result<(), Error> {
        let request = Request {
            url: format!("{}/{id}?version={version}", self.url()),
            bearer: access_token.to_owned(),
            method: Method::Delete,
            body: None,
            idempotency_key: None,
        };
        match send(transport, &request) {
            // The service answers a delete of an unknown record with a `200` and no tombstone; the
            // `404` arm is kept for a deployment that has not caught up. A refusal because the
            // record is already a tombstone is the removal having happened everywhere already.
            Ok(_)
            | Err(
                Error::Unexpected { status: 404 }
                | Error::Conflict(Some(ConflictWith::Tombstone(_))),
            ) => Ok(()),
            Err(other) => Err(other),
        }
    }
}

/// Send one request and turn its status into this crate's errors.
fn send(transport: &dyn Transport, request: &Request) -> Result<String, Error> {
    let response = transport.send(request).map_err(Error::Transport)?;
    match response.status {
        200..=299 => Ok(response.body),
        401 | 403 => Err(Error::Unauthorized),
        409 => Err(conflict(&response.body)),
        // A refusal is one the service wrote with a code, which a `400` over a payload it could
        // not accept does not carry.
        400 => {
            Err(Refusal::read(&response.body)
                .map_or(Error::Unexpected { status: 400 }, Error::Refused))
        }
        status => Err(Error::Unexpected { status }),
    }
}

fn parse<T: DeserializeOwned>(body: &str) -> Result<T, Error> {
    serde_json::from_str(body).map_err(|error| Error::Malformed(error.to_string()))
}

/// Read a `409` body, falling back to a bare conflict when it cannot be parsed.
///
/// A conflict whose payload is unreadable is still a conflict: reporting it as malformed would send
/// the caller down the "the service is broken" path when the answer is "re-read and try again".
fn conflict(body: &str) -> Error {
    let current = serde_json::from_str::<ConflictBody>(body)
        .ok()
        .and_then(|parsed| parsed.data.current);
    Error::Conflict(current)
}

/// A write's body: the version it read, when it names one, and the payload under its key.
fn wrap<R: SyncedRecord>(version: Option<u64>, payload: &R::Payload) -> Result<String, Error> {
    let malformed = |error: serde_json::Error| Error::Malformed(error.to_string());
    let mut body = serde_json::Map::new();
    if let Some(version) = version {
        body.insert("version".to_owned(), version.into());
    }
    body.insert(
        R::PAYLOAD.to_owned(),
        serde_json::to_value(payload).map_err(malformed)?,
    );
    serde_json::to_string(&body).map_err(malformed)
}

/// Percent-encode a query value.
///
/// Only the characters a timestamp can contain need escaping, and `+` is the one that matters: it
/// means a space to a form decoder, so a `+02:00` offset arrives as a space and the delta silently
/// covers the wrong window.
fn encode_query(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            other => {
                let _ = write!(encoded, "%{other:02X}");
            }
        }
    }
    encoded
}
