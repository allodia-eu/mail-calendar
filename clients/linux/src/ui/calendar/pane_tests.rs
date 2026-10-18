//! What the calendar header's write badge says for each write status.

use mailcal_bindings::CalendarWriteStatus;

use super::status_badge;
use crate::l10n;

#[test]
fn every_write_status_has_its_own_badge_and_only_a_failure_offers_refresh() {
    assert_eq!(status_badge(CalendarWriteStatus::Idle), None);
    assert_eq!(
        status_badge(CalendarWriteStatus::Saving),
        Some((l10n::calendar_saving(), false))
    );
    assert_eq!(
        status_badge(CalendarWriteStatus::Saved),
        Some((l10n::calendar_saved(), false))
    );
    assert_eq!(
        status_badge(CalendarWriteStatus::NotSaved),
        Some((l10n::calendar_save_failed(), true))
    );
    assert_eq!(
        status_badge(CalendarWriteStatus::Unconfirmed),
        Some((l10n::calendar_save_unconfirmed(), true))
    );
}
