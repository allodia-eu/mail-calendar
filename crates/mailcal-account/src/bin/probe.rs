//! `probe`; connect to a real IMAP account from a config file, sync its Inbox + Sent
//! folders through an on-disk engine and report what synced (+ a CalDAV
//! calendar sync when configured). It proves the engine ⇄ provider path; including
//! cross-folder threading; against a *real* server (e.g. Soverin) rather than the
//! harness, which is what the native apps otherwise exercise.
//!
//! ```sh
//! cargo run -p mailcal-account --bin probe -- [<config.toml>] [-v]
//! ```
//!
//! Reads `$HOME/.config/mailcal/account.toml` by default. Prints counts + thread stats
//! only (no subjects); pass `-v` to also print folder names.

use std::path::PathBuf;

use engine_api::{AccountId, Engine, Horizon, IgnoreCommits, StreamTuning, TimeZoneId};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config_path: Option<PathBuf> = None;
    let mut verbose = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-v" | "--verbose" => verbose = true,
            other => config_path = Some(PathBuf::from(other)),
        }
    }
    let config_path = config_path.unwrap_or_else(mailcal_account::default_path);

    eprintln!("Loading account config from {}", config_path.display());
    let config = mailcal_account::load(&config_path)?;

    // A throwaway on-disk store (a real host uses its app-group/container path).
    let db = std::env::temp_dir().join("mailcal-probe.sqlite3");
    let _ = std::fs::remove_file(&db);
    let engine = Engine::open(&db)?;
    let account = AccountId::try_from("probe")?;

    let security = match config.imap.security {
        mailcal_account::ConnectionSecurity::ImplicitTls => "implicit TLS",
        mailcal_account::ConnectionSecurity::StartTls => "STARTTLS",
    };
    eprintln!(
        "Connecting to {} as {} ({security}, verifying)…",
        config.imap.addr, config.imap.username
    );
    // The probe syncs the whole mailbox (no sync-depth window) for the de-risk run.
    let providers = mailcal_account::connect_mail_providers(&config, &account, None).await?;

    eprintln!("Syncing {} folder(s) (Inbox + Sent)…", providers.len());
    // One pass over the whole account: the engine syncs the folder list once and fans the
    // folders out itself, which is what the app does too.
    let report = engine
        .sync_mail(
            &providers,
            &account,
            StreamTuning::default(),
            &IgnoreCommits,
        )
        .await;
    if let Some(err) = report.first_error() {
        eprintln!("  (a scope did not sync: {err})");
    }

    // The sync threaded what it applied, so this is an assertion rather than a step: a rebuild
    // with anything left to assign means the incremental path missed something.
    eprintln!("Checking the thread index…");
    let rebuilt = engine.rebuild_thread_index(&account).await?;

    let mailboxes = engine.mailboxes(&account).await?;
    let messages = engine.messages(&account).await?;

    // Thread stats only (no subjects). A thread spanning >1 message shows threading
    // working; a multi-message thread that pulls in a Sent reply is the sent/received
    // grouping the user asked for.
    let mut sizes: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for message in &messages {
        let key = message.thread_id().map_or_else(
            || message.id.key().as_str().to_owned(),
            |thread| thread.as_str().to_owned(),
        );
        *sizes.entry(key).or_default() += 1;
    }
    let multi = sizes.values().filter(|count| **count > 1).count();
    let largest = sizes.values().copied().max().unwrap_or(0);

    println!(
        "\n✓ synced {} folders, {} messages → {} threads \
         ({} multi-message, largest {} messages); rebuild reassigned {}",
        mailboxes.len(),
        messages.len(),
        sizes.len(),
        multi,
        largest,
        rebuilt.messages_assigned,
    );

    if verbose {
        println!("\nfolders:");
        for mailbox in &mailboxes {
            println!("  {}", mailbox.name);
        }
    } else {
        println!("(run with -v to print folder names)");
    }

    // Calendar sync, when the account configures a CalDAV endpoint. Mirrors the mail
    // path: connect the provider, drive the engine, report counts only (content stays
    // out of stdout unless -v).
    if let Some(caldav) = &config.caldav {
        eprintln!(
            "\nConnecting to {} as {} (https, verifying)…",
            caldav.base_url, caldav.username
        );
        let provider = mailcal_account::connect_caldav(&config).await?;

        // A one-year materialization window, with floating times resolved through the
        // user's home zone. Both are fixed here because this is a probe (a real host
        // derives them from the device): a calendar year covers the common "this year"
        // view, and Europe/Amsterdam is the user's zone.
        let horizon = Horizon::new(
            "2026-01-01T00:00:00Z".parse()?,
            "2027-01-01T00:00:00Z".parse()?,
        )?;
        let host_zone = TimeZoneId::iana("Europe/Amsterdam")?;

        eprintln!("Syncing calendar (calendar list + events)…");
        let report = engine
            .sync_calendar(&provider, &account, horizon, &host_zone)
            .await?;

        println!(
            "\n✓ synced {} calendars, {} calendar events",
            report.calendars.upserted, report.events.applied.upserted
        );
        if verbose {
            println!(
                "  (events: {} upserted, {} tombstoned; calendars: {} upserted, {} tombstoned)",
                report.events.applied.upserted,
                report.events.applied.tombstoned,
                report.calendars.upserted,
                report.calendars.tombstoned,
            );
            // engine-api exposes no calendar/event read surface (no calendars()/
            // events() to mirror mailboxes()/messages()), so per-event detail is not
            // available here without a read-back method. See the gap note in the
            // hand-off.
        }
    }
    Ok(())
}
