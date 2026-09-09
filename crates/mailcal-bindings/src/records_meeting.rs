//! Meeting invitee records accepted by calendar write intents.

use engine_api::{CalendarAddress, Invitee, InviteePatch, SchedulingIdentity};

/// Whether attendance is requested or optional.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum MeetingInviteeRole {
    /// Attendance is requested.
    Required,
    /// Attendance is optional.
    Optional,
}

/// One person to invite to a meeting.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MeetingInvitee {
    /// The display name, if known.
    pub name: Option<String>,
    /// The email calendar address.
    pub email: String,
    /// Whether attendance is requested or optional.
    pub role: MeetingInviteeRole,
}

/// Invitee changes for an existing meeting.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MeetingInviteePatch {
    /// Invitees to add, or whose name or role changed.
    pub upsert: Vec<MeetingInvitee>,
    /// Addresses whose invitations to cancel.
    pub remove: Vec<String>,
}

impl TryFrom<MeetingInvitee> for Invitee {
    type Error = String;

    fn try_from(invitee: MeetingInvitee) -> Result<Self, Self::Error> {
        let address = CalendarAddress::parse(&invitee.email).map_err(|err| err.to_string())?;
        let name = invitee
            .name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty());
        let identity = match name {
            Some(name) => SchedulingIdentity::named(address, name),
            None => SchedulingIdentity::new(address),
        };
        Ok(match invitee.role {
            MeetingInviteeRole::Required => Self::required(identity),
            MeetingInviteeRole::Optional => Self::optional(identity),
        })
    }
}

impl TryFrom<MeetingInviteePatch> for InviteePatch {
    type Error = String;

    fn try_from(changes: MeetingInviteePatch) -> Result<Self, Self::Error> {
        let mut patch = Self::new();
        for invitee in changes.upsert {
            patch = patch.upsert(invitee.try_into()?);
        }
        for address in changes.remove {
            let address = CalendarAddress::parse(&address).map_err(|err| err.to_string())?;
            patch = patch.remove(address);
        }
        Ok(patch)
    }
}

pub(crate) fn invitees(
    values: Option<Vec<MeetingInvitee>>,
) -> Result<Option<Vec<Invitee>>, String> {
    values
        .map(|values| values.into_iter().map(TryInto::try_into).collect())
        .transpose()
}

pub(crate) fn invitee_patch(
    value: Option<MeetingInviteePatch>,
) -> Result<Option<InviteePatch>, String> {
    value.map(TryInto::try_into).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Intent;

    fn required(email: &str, name: &str) -> MeetingInvitee {
        MeetingInvitee {
            name: Some(name.to_owned()),
            email: email.to_owned(),
            role: MeetingInviteeRole::Required,
        }
    }

    fn create(invitees: Vec<MeetingInvitee>) -> Intent {
        Intent::CreateEvent {
            title: "Planning".to_owned(),
            start: "2026-09-14T09:00:00Z".to_owned(),
            end: "2026-09-14T10:00:00Z".to_owned(),
            account: Some("work".to_owned()),
            calendar: None,
            all_day: false,
            timezone: None,
            notes: None,
            location: None,
            recurrence: None,
            invitees: Some(invitees),
        }
    }

    #[test]
    fn a_create_carries_validated_invitees_across_the_boundary() {
        let converted =
            mailcal_app::Intent::try_from(create(vec![required("Grace@Example.com", "Grace")]))
                .expect("a valid invitee converts");

        let mailcal_app::Intent::CreateEvent { invitees, .. } = converted else {
            panic!("expected a CreateEvent");
        };
        let invitees = invitees.expect("the event is a meeting");
        assert_eq!(
            invitees[0].identity().address().as_str(),
            "Grace@example.com"
        );
        assert_eq!(invitees[0].identity().name(), Some("Grace"));
        assert_eq!(invitees[0].role(), engine_api::InviteeRole::Required);
    }

    #[test]
    fn an_invalid_invitee_address_drops_the_create() {
        let converted =
            mailcal_app::Intent::try_from(create(vec![required("not an address", "Nobody")]));

        assert!(converted.is_err());
    }

    #[test]
    fn an_update_carries_additions_and_removals() {
        let converted = mailcal_app::Intent::try_from(Intent::UpdateEvent {
            account: "work".to_owned(),
            key: "planning.ics".to_owned(),
            title: None,
            start: None,
            end: None,
            notes: None,
            location: None,
            occurrence: None,
            recurrence: None,
            times_from_occurrence: None,
            invitees: Some(MeetingInviteePatch {
                upsert: vec![MeetingInvitee {
                    name: None,
                    email: "optional@example.com".to_owned(),
                    role: MeetingInviteeRole::Optional,
                }],
                remove: vec!["old@example.com".to_owned()],
            }),
        })
        .expect("a valid roster patch converts");

        let mailcal_app::Intent::UpdateEvent { edit, .. } = converted else {
            panic!("expected an UpdateEvent");
        };
        let changes = edit.invitees.expect("the roster changes");
        assert_eq!(
            changes.upserts()[0].role(),
            engine_api::InviteeRole::Optional
        );
        assert_eq!(changes.removals()[0].as_str(), "old@example.com");
    }
}
