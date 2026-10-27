//! Every calendar a person sees is drawn in a colour no other calendar has, across all their
//! accounts, while hues last: a server's colour is kept unless an earlier calendar holds its hue.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use super::{
    Intent,
    fakes::{CalendarFake, calendar_account, calendar_app},
};

/// Each calendar's resolved colour, by calendar id.
async fn colors(accounts: Vec<(&str, CalendarFake)>) -> Vec<(String, String)> {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let accounts = accounts
        .into_iter()
        .map(|(id, provider)| calendar_account(id, provider))
        .collect();
    let app = calendar_app(accounts, &surfaces);
    app.dispatch(Intent::RefreshCalendar).await;
    app.calendars()
        .into_iter()
        .map(|row| (row.id, row.color.hex))
        .collect()
}

fn distinct(colors: &[(String, String)]) -> bool {
    colors
        .iter()
        .map(|(_, hex)| hex)
        .collect::<HashSet<_>>()
        .len()
        == colors.len()
}

/// iCloud sends `#RRGGBBAA`: four calendars, four colours, each the server's own.
#[tokio::test]
async fn a_server_colour_with_an_alpha_channel_is_the_calendars_colour() {
    let colors = colors(vec![(
        "icloud",
        CalendarFake::with_server_colors(&[
            ("home", Some("#F09650FF")),
            ("work", Some("#0E61B9FF")),
            ("family", Some("#34AADCFF")),
            ("trips", Some("#CC73E1FF")),
        ]),
    )])
    .await;
    assert!(distinct(&colors), "{colors:?}");
}

/// Yahoo sends one colour for every calendar, and another account can send the same one: the
/// first calendar keeps it and each later one takes a hue nobody holds.
#[tokio::test]
async fn calendars_sharing_a_server_colour_are_drawn_apart_across_accounts() {
    let colors = colors(vec![
        (
            "yahoo",
            CalendarFake::with_server_colors(&[
                ("yahoo-1", Some("#492BA1")),
                ("yahoo-2", Some("#492BA1")),
            ]),
        ),
        (
            "fastmail",
            CalendarFake::with_server_colors(&[("fm-1", None), ("fm-2", Some("#492ba1"))]),
        ),
    ])
    .await;
    assert!(distinct(&colors), "{colors:?}");
    let alone = colors_of_one("#492BA1").await;
    assert_eq!(
        colors[0].1, alone,
        "the first calendar keeps its server's colour"
    );
}

async fn colors_of_one(hex: &str) -> String {
    colors(vec![(
        "solo",
        CalendarFake::with_server_colors(&[("only", Some(hex))]),
    )])
    .await
    .remove(0)
    .1
}
