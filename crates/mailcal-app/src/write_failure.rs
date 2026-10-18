//! What a calendar or contact write the engine returned an error for did to the server.
//!
//! The person is told one of two true things: nothing was saved, or the change may have been and
//! the next sync settles which. The engine marks a failure whose outcome is unknown (the request
//! may have arrived and no answer was read) as one that needs confirmation; every other
//! failure, a refusal from the server or one before anything was sent, saved nothing.

use std::error::Error as StdError;

use engine_api::ApiError;

use crate::{CalendarWriteStatus, ContactWriteStatus, connectivity::provider_error_of};

/// Whether the write behind `error` may have been applied.
pub(crate) fn may_have_applied(error: &ApiError) -> bool {
    match error {
        // Refused before anything was sent: another write held the scope, or the input was bad.
        ApiError::Busy | ApiError::InvalidInput(_) => false,
        other => {
            let error: &(dyn StdError + 'static) = other;
            provider_error_of(error)
                .is_none_or(engine_provider::ProviderError::requires_confirmation)
        }
    }
}

/// The calendar status a failed write settles.
pub(crate) fn calendar_status(error: &ApiError) -> CalendarWriteStatus {
    if may_have_applied(error) {
        CalendarWriteStatus::Unconfirmed
    } else {
        CalendarWriteStatus::NotSaved
    }
}

/// The contact status a failed write settles.
pub(crate) fn contact_status(error: &ApiError) -> ContactWriteStatus {
    if may_have_applied(error) {
        ContactWriteStatus::Unconfirmed
    } else {
        ContactWriteStatus::NotSaved
    }
}
