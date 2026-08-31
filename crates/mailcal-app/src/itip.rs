//! Writing the two iCalendar documents this core has to produce itself.
//!
//! The engine parses iCalendar and patches it, but it has no `Event` → iTIP **serializer**, and
//! [it deliberately will not grow one](https://github.com/allodia-eu/email-calendar-sync-engine)
//! for this: an answer keys to a `UID`/`SEQUENCE` the caller holds, and the projection an
//! engine `Event` carries is lossy in exactly the places a scheduling message must be exact.
//! So the caller supplies the text, the same way it supplies the document a `put_event` stores.
//! Here is that caller.
//!
//! Two documents, for the two halves of answering an invitation on an account whose calendar
//! server does no scheduling of its own (`docs/invitations.md` → "Who delivers the answer"):
//!
//! - [`reply`] builds the `METHOD:REPLY` object that **tells the organiser** (RFC 5546 §3.2.3),
//!   carried as an iMIP body part (RFC 6047 §2.4).
//! - [`for_storage`] strips the `METHOD` property off the invitation as it arrived, so the same
//!   bytes can be **stored** as a calendar object resource; RFC 4791 §4.1 forbids `METHOD` on a
//!   stored resource, because it is a property of a message in transit, not of a meeting.
//!
//! # Why a writer at all, rather than editing the invitation
//!
//! A `REPLY` is not a `REQUEST` with a changed `PARTSTAT`. RFC 5546 §3.2.3 says it carries the
//! organiser and **only the replying attendee**; sending back the full attendee list would
//! leak every other invitee's answer to a server that never had it, and an organiser's
//! scheduler is entitled to read the whole list as authoritative. So the reply is assembled
//! from a handful of named fields, not derived by patching, and this module cannot emit a
//! second `ATTENDEE` even by accident.
//!
//! **Privacy.** Nothing here logs. A `SUMMARY`, an organiser address and an attendee's own note
//! are precisely the content `docs/logging.md` forbids in the diagnostic log.

use engine_api::{ParticipationStatus, UtcDateTime};

/// The `PRODID` every document from this module carries (RFC 5545 §3.7.3).
///
/// A fixed, versionless string on purpose: it identifies the software to a human reading a
/// bounced scheduling message, and a version in it would make every release's output differ
/// from the fixtures that pin this format.
const PRODID: &str = "-//Allodia//Allodia Mail & Calendar//EN";

/// Everything the `METHOD:REPLY` needs, resolved. Pure data: the caller has already read the
/// calendar, matched the alias, and turned the meeting's start into an instant.
#[derive(Debug, Clone)]
pub(crate) struct Reply<'a> {
    /// The meeting's cross-system `UID`, copied verbatim from the invitation. The organiser's
    /// scheduler matches on this and nothing else.
    pub uid: &'a str,
    /// The `SEQUENCE` **of the invitation being answered**, so an organiser who has already
    /// moved on can tell that this reply answers a revision they superseded (RFC 5546 §2.1.5).
    pub sequence: u32,
    /// The organiser's calendar address, as the invitation wrote it.
    pub organizer: &'a str,
    /// The address this account is answering **as**: the one the invitation matched, which on
    /// an aliased account is not the account's primary identity (`docs/invitations.md` §4).
    pub attendee: &'a str,
    /// The attendee's display name for the `CN` parameter, if the invitation carried one.
    pub attendee_name: Option<&'a str>,
    /// The answer.
    pub status: ParticipationStatus,
    /// The meeting's start, as an instant. Emitted as a UTC `DTSTART`, which is optional in a
    /// `REPLY` (RFC 5546 §3.2.3) but helps a client that matches loosely.
    pub starts_at: UtcDateTime,
    /// The instance being answered, when the invitation named one. **Load-bearing**: a reply
    /// that drops a `RECURRENCE-ID` answers the whole series instead of the one occurrence.
    pub recurrence_id: Option<UtcDateTime>,
    /// When this reply was composed (`DTSTAMP`), which is what orders two answers to the same
    /// revision.
    pub stamp: UtcDateTime,
    /// The user's note to the organiser, if they wrote one.
    pub comment: Option<&'a str>,
}

/// Builds the `METHOD:REPLY` iCalendar object for one answer.
///
/// The component set is RFC 5546 §3.2.3's: `ORGANIZER`, exactly one `ATTENDEE` (the replier,
/// carrying the new `PARTSTAT`), `UID`, `SEQUENCE`, `DTSTAMP`, and, where they apply;
/// `RECURRENCE-ID` and `COMMENT`.
///
/// Every line is escaped and folded on the way out ([`fold`]), so a note with a comma in it, or
/// a `UID` long enough to overrun 75 octets, produces a document a strict parser still reads.
pub(crate) fn reply(reply: &Reply<'_>) -> String {
    let mut out = String::new();
    out.push_str("BEGIN:VCALENDAR\r\n");
    line(&mut out, &format!("PRODID:{PRODID}"));
    line(&mut out, "VERSION:2.0");
    line(&mut out, "METHOD:REPLY");
    line(&mut out, "BEGIN:VEVENT");
    line(&mut out, &format!("UID:{}", escape_text(reply.uid)));
    line(&mut out, &format!("DTSTAMP:{}", stamp(reply.stamp)));
    line(&mut out, &format!("DTSTART:{}", stamp(reply.starts_at)));
    if let Some(instance) = reply.recurrence_id {
        line(&mut out, &format!("RECURRENCE-ID:{}", stamp(instance)));
    }
    line(&mut out, &format!("SEQUENCE:{}", reply.sequence));
    line(
        &mut out,
        &format!("ORGANIZER:{}", cal_address(reply.organizer)),
    );
    line(&mut out, &attendee_line(reply));
    if let Some(comment) = reply.comment.map(str::trim).filter(|text| !text.is_empty()) {
        line(&mut out, &format!("COMMENT:{}", escape_text(comment)));
    }
    line(&mut out, "END:VEVENT");
    out.push_str("END:VCALENDAR\r\n");
    out
}

/// The one `ATTENDEE` line: the replier, their answer, and their name if the invitation knew it.
fn attendee_line(reply: &Reply<'_>) -> String {
    let mut out = String::from("ATTENDEE");
    if let Some(name) = reply.attendee_name.filter(|name| !name.trim().is_empty()) {
        out.push_str(";CN=");
        out.push_str(&quote_param(name));
    }
    out.push_str(";PARTSTAT=");
    // The engine spells a participation status the JSCalendar way (lowercase, `needs-action`);
    // iCalendar `PARTSTAT` values are uppercase (RFC 5545 §3.2.12): the exact inverse of how
    // the parser lowercases on read, and the same conversion `imip::set_my_partstat` makes.
    out.push_str(&reply.status.as_str().to_ascii_uppercase());
    out.push(':');
    out.push_str(&cal_address(reply.attendee));
    out
}

/// Strips the `METHOD` property from a calendar object so the same bytes can be **stored**.
///
/// RFC 4791 §4.1: a calendar object resource "MUST NOT specify the iCalendar `METHOD`
/// property". `METHOD` describes a message in transit; *this is a request*, *this is a reply*;
/// and a stored meeting is neither. A server is entitled to reject the `PUT` outright, and
/// Sabre/DAV does.
///
/// Everything else survives **byte for byte**, including the `VTIMEZONE` the organiser sent,
/// their `X-` properties, and the `ATTENDEE` lines that are the whole point of storing the
/// invitation rather than a plain appointment. That is why this is line surgery on the wire
/// form rather than a re-serialization of the parsed projection: a round-trip through the
/// projection would quietly drop whatever it does not model, and the answer would then be
/// written against a meeting missing pieces the organiser sent.
///
/// Returns `None` if `raw` carries no `BEGIN:VCALENDAR` at all: not a calendar object, so not
/// something to store.
pub(crate) fn for_storage(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut depth: usize = 0;
    let mut dropping = false;
    let mut saw_calendar = false;

    for physical in raw.split_inclusive('\n') {
        // A line beginning with a space or a horizontal tab is a continuation of the one
        // before it (RFC 5545 §3.1), so it belongs to whatever that line's decision was.
        if physical.starts_with([' ', '\t']) {
            if !dropping {
                out.push_str(physical);
            }
            continue;
        }
        dropping = false;
        let name = property_name(physical);
        if name.eq_ignore_ascii_case("BEGIN") {
            saw_calendar |= component_of(physical).eq_ignore_ascii_case("VCALENDAR");
            depth += 1;
        } else if name.eq_ignore_ascii_case("END") {
            depth = depth.saturating_sub(1);
        } else if depth == 1 && name.eq_ignore_ascii_case("METHOD") {
            // Only at the calendar level. `METHOD` is defined nowhere else, but bounding it by
            // depth means an `X-` component that invented one cannot lose a line here.
            dropping = true;
            continue;
        }
        out.push_str(physical);
    }
    saw_calendar.then_some(out)
}

/// The property name of a physical line: everything before the first `;` or `:`.
fn property_name(line: &str) -> &str {
    let end = line.find([';', ':']).unwrap_or(line.len());
    line[..end].trim_end_matches(['\r', '\n'])
}

/// The component a `BEGIN:` line opens.
fn component_of(line: &str) -> &str {
    line.split_once(':')
        .map_or("", |(_, value)| value.trim_end_matches(['\r', '\n']))
}

/// Formats an instant as an iCalendar UTC date-time (RFC 5545 §3.3.5 form 2).
fn stamp(at: UtcDateTime) -> String {
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        at.year(),
        at.month(),
        at.day(),
        at.hour(),
        at.minute(),
        at.second(),
    )
}

/// Renders a calendar address as an iTIP `mailto:` URI.
///
/// A `CAL-ADDRESS` value is a URI, not text, so it is **not** `TEXT`-escaped: a backslash in
/// one stays a backslash. The invitation may already have written the scheme; adding a second
/// one would produce `mailto:mailto:…`, which matches nobody.
fn cal_address(address: &str) -> String {
    let bare = address
        .strip_prefix("mailto:")
        .or_else(|| address.strip_prefix("MAILTO:"))
        .unwrap_or(address);
    format!("mailto:{bare}")
}

/// Escapes a `TEXT` value (RFC 5545 §3.3.11).
///
/// The backslash goes first, or escaping it again would double the escapes this function just
/// added. A carriage return is dropped rather than turned into its own `\n`, so a CRLF inside a
/// note does not become two blank lines.
fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            other => out.push(other),
        }
    }
    out
}

/// Renders a property **parameter** value, quoting it when it needs quoting (RFC 5545 §3.1).
///
/// Parameter values use a different rule from `TEXT`: there is no backslash escape at all, so a
/// value containing `:`, `;` or `,` must be wrapped in double quotes, and a value containing a
/// double quote **cannot be represented**, which is why one is dropped rather than emitted. The
/// only thing that reaches here is a display name the organiser chose, so losing a quote
/// character from it costs nothing; emitting one would end the parameter early and corrupt the
/// line.
fn quote_param(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .filter(|ch| *ch != '"' && *ch != '\r' && *ch != '\n')
        .collect();
    if cleaned.contains([':', ';', ',']) {
        format!("\"{cleaned}\"")
    } else {
        cleaned
    }
}

/// Appends one logical line, folded to the octet limit and CRLF-terminated.
fn line(out: &mut String, logical: &str) {
    out.push_str(&fold(logical));
    out.push_str("\r\n");
}

/// The octet limit a content line is folded at (RFC 5545 §3.1: 75 octets **excluding** the line
/// break).
const FOLD_LIMIT: usize = 75;

/// Folds a logical line to [`FOLD_LIMIT`] octets per physical line, continuing with CRLF + a
/// single space.
///
/// Measured in **octets, not characters**: the RFC counts bytes, and a document of Cyrillic
/// summaries folded by character count produces lines over twice the limit. But a break may
/// only fall on a character boundary, so this walks `char_indices` and cuts at the last one
/// that fits: splitting a multi-byte character across a fold would corrupt it beyond repair,
/// since a parser unfolds by deleting the CRLF-and-space before it decodes anything.
fn fold(logical: &str) -> String {
    if logical.len() <= FOLD_LIMIT {
        return logical.to_owned();
    }
    let mut out = String::with_capacity(logical.len() + logical.len() / FOLD_LIMIT * 3);
    let mut rest = logical;
    // The first physical line carries the full budget; every continuation spends one octet on
    // the leading space that marks it as one.
    let mut budget = FOLD_LIMIT;
    while rest.len() > budget {
        let cut = rest
            .char_indices()
            .map(|(index, _)| index)
            .take_while(|index| *index <= budget)
            .last()
            .unwrap_or(rest.len());
        // A single character wider than the remaining budget would otherwise cut at 0 and loop
        // forever; emit it whole and overrun by a few octets instead, which every parser
        // tolerates and an infinite loop does not.
        let cut = if cut == 0 {
            rest.char_indices().nth(1).map_or(rest.len(), |(at, _)| at)
        } else {
            cut
        };
        out.push_str(&rest[..cut]);
        out.push_str("\r\n ");
        rest = &rest[cut..];
        budget = FOLD_LIMIT - 1;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
#[path = "itip_tests.rs"]
mod tests;
