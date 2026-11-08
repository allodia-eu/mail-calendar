//! The mail half of an account's page (`docs/settings.md`): the name it is sent under, how its mail
//! arrives and how much of it this device keeps, drawn from the account's sync row. Which folders
//! may still be watched, which count is in effect and where a running change takes its control
//! are decided here, so no client keeps a copy.

use mailcal_account::{MAX_PUSH_FOLDERS, MESSAGE_SIZE_LIMITS_MB, POLL_INTERVALS, SYNC_DEPTHS};

use crate::{
    AccountCountChoice, AccountMailSettings, AccountSenderName, AccountSyncRow, AccountToggle,
    AccountWatchedFolder, ControlState, SyncStrategyKind,
    account_work::{Hold, PageWork},
};

/// The sender name and mail settings for `row`'s account, held as `hold` says.
pub(crate) fn mail_half(
    row: AccountSyncRow,
    hold: &Hold,
) -> (AccountSenderName, AccountMailSettings) {
    let control = |running: bool, free: bool| ControlState {
        enabled: free && !hold.busy,
        running,
    };
    let naming = hold.going(|work| match work {
        PageWork::SenderName(name) => Some(name.clone()),
        _ => None,
    });
    let editable = row.sender_name_editable;
    let sender_name = AccountSenderName {
        control: control(naming.is_some(), editable),
        name: match naming {
            Some(name) => name,
            None if editable || !row.sender_name.is_empty() => row.sender_name,
            None => row.email,
        },
        editable,
    };

    let count = |options: &[u16], value: u16, going: Option<u16>| {
        let value = going.unwrap_or(value);
        let mut options = options.to_vec();
        let at = options
            .iter()
            .position(|option| *option == value)
            .unwrap_or_else(|| {
                // A count stored before the list changed is still the one in effect, so it is
                // offered rather than drawn as another.
                options.push(value);
                options.len() - 1
            });
        AccountCountChoice {
            options,
            selected: u32::try_from(at).unwrap_or(0),
            control: control(going.is_some(), true),
        }
    };
    let depth = count(
        &SYNC_DEPTHS,
        row.sync_depth_months,
        hold.going(|work| match work {
            PageWork::Depth(months) => Some(*months),
            _ => None,
        }),
    );
    let size = count(
        &MESSAGE_SIZE_LIMITS_MB,
        row.message_size_limit_mb,
        hold.going(|work| match work {
            PageWork::Size(megabytes) => Some(*megabytes),
            _ => None,
        }),
    );

    let pushing_to = hold.going(|work| match work {
        PageWork::Push(on) => Some(*on),
        _ => None,
    });
    let pushes = row.idle_supported && pushing_to.unwrap_or(row.strategy == SyncStrategyKind::Push);
    let push = row.idle_supported.then(|| AccountToggle {
        on: pushes,
        control: control(pushing_to.is_some(), true),
    });
    let interval = (!pushes).then(|| {
        count(
            &POLL_INTERVALS,
            row.poll_interval_mins,
            hold.going(|work| match work {
                PageWork::Interval(minutes) => Some(*minutes),
                _ => None,
            }),
        )
    });
    let folders = if pushes {
        row.folders
            .into_iter()
            .map(|folder| {
                let going = hold.going(|work| match work {
                    PageWork::Folder { key, on } if *key == folder.key => Some(*on),
                    _ => None,
                });
                let on = going.unwrap_or(folder.subscribed);
                AccountWatchedFolder {
                    control: control(going.is_some(), on || !row.at_push_limit),
                    key: folder.key,
                    name: folder.name,
                    role: folder.role,
                    on,
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    (
        sender_name,
        AccountMailSettings {
            depth,
            size,
            push,
            interval,
            folders,
            watch_limit: u8::try_from(MAX_PUSH_FOLDERS).unwrap_or(u8::MAX),
        },
    )
}

#[cfg(test)]
#[path = "account_page_mail_tests.rs"]
mod tests;
