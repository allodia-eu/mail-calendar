//! The Sign-in section as the core decides it: the line, the ways in each kind of account offers
//! once its server has answered, the password field, and the answer kept until the account's way
//! in changes.

use AccountSignInLine::{AtProvider, NeedsPermission, Nextcloud, Password, Provider, Refused};
use AccountSignInRoute::{SignInAgain, ToNextcloud, ToProvider};

use super::{SignInAnswers, section};
use crate::{
    AccountCapability, AccountEndpoints, AccountEntry, AccountKind, AccountLinksView,
    AccountPasswordField, AccountSecretKind, AccountSignIn, AccountSignInLine,
    AccountSignInOptions, AccountSignInRoute, AccountUse, CapabilityState, ConnectionSecurity,
    ControlState, LinkCandidates, SignInMethod,
    account_work::{Hold, PageWork},
};

const IDLE: ControlState = ControlState {
    enabled: true,
    running: false,
};

fn entry(kind: AccountKind, sign_in: SignInMethod) -> AccountEntry {
    AccountEntry {
        id: "alice@example.org@imap.example.org".to_owned(),
        address: "alice@example.org".to_owned(),
        kind,
        sign_in,
        uses: vec![AccountUse {
            capability: AccountCapability::Mail,
            state: CapabilityState::On,
            problem: None,
        }],
        links: AccountLinksView::default(),
        linked_from: Vec::new(),
        link_candidates: LinkCandidates::default(),
        endpoints: None,
        separate_server: None,
        distinction: None,
        mail_sync: None,
        nextcloud: false,
    }
}

fn editable(mut entry: AccountEntry) -> AccountEntry {
    entry.endpoints = Some(AccountEndpoints {
        username: "alice".to_owned(),
        password: None,
        imap_host: Some("imap.example.org:993".to_owned()),
        imap_security: ConnectionSecurity::ImplicitTls,
        smtp_host: None,
        smtp_security: ConnectionSecurity::ImplicitTls,
        caldav_url: None,
        carddav_url: None,
    });
    entry
}

fn options(sign_in: bool, password: bool, nextcloud: bool) -> AccountSignInOptions {
    AccountSignInOptions {
        sign_in,
        provider_label: None,
        password,
        nextcloud,
    }
}

fn drawn(
    entry: &AccountEntry,
    answer: Option<&AccountSignInOptions>,
    refused: bool,
) -> AccountSignIn {
    section(entry, answer, refused, &Hold::default()).expect("a section")
}

fn field(kind: AccountSecretKind, replaces: bool) -> Option<AccountPasswordField> {
    Some(AccountPasswordField {
        kind,
        replaces,
        save: IDLE,
    })
}

#[test]
fn microsoft_and_google_sign_in_again_and_say_what_is_wrong() {
    let mut account = entry(AccountKind::Microsoft, SignInMethod::Provider);
    let ok = drawn(&account, None, false);
    assert_eq!(ok.line, AtProvider);
    assert_eq!(ok.routes, [SignInAgain]);
    assert_eq!((ok.password, ok.switches), (None, false));

    account.uses[0].state = CapabilityState::NeedsPermission;
    assert_eq!(drawn(&account, None, false).line, NeedsPermission);
    assert_eq!(drawn(&account, None, true).line, Refused);
}

#[test]
fn a_provider_account_offers_nothing_until_its_server_answers() {
    let account = entry(AccountKind::Imap, SignInMethod::Provider);
    let asking = drawn(&account, None, false);
    assert_eq!(asking.line, Provider);
    assert!(asking.routes.is_empty());
    assert_eq!(asking.password, None);

    let answered = drawn(&account, Some(&options(true, true, false)), false);
    assert_eq!(answered.routes, [SignInAgain]);
    assert_eq!(answered.password, field(AccountSecretKind::Password, false));
    assert!(answered.switches);

    // Refused, it says so where its remedy is offered.
    assert_eq!(
        drawn(&account, Some(&options(true, false, false)), true).line,
        Refused
    );
    assert!(!drawn(&account, Some(&options(true, false, false)), true).switches);
}

#[test]
fn a_password_account_switches_to_its_provider_named_when_the_build_knows_it() {
    let account = entry(AccountKind::Imap, SignInMethod::Password);
    let mut answer = options(true, true, false);
    answer.provider_label = Some("Fastmail".to_owned());
    let named = drawn(&account, Some(&answer), false);
    assert_eq!(named.line, Password);
    assert_eq!(
        named.routes,
        [ToProvider {
            label: Some("Fastmail".to_owned())
        }]
    );
    assert!(named.switches);
    // A password account never offers the password it already signs in with.
    assert_eq!(named.password, None);

    answer.provider_label = Some(String::new());
    assert_eq!(
        drawn(&account, Some(&answer), false).routes,
        [ToProvider { label: None }]
    );
    assert!(
        drawn(&account, Some(&options(false, true, false)), false)
            .routes
            .is_empty()
    );
}

#[test]
fn a_refused_password_is_typed_where_the_page_offers_it() {
    // Its servers are edited on the page, so the new password goes in with them.
    let imap = editable(entry(AccountKind::Imap, SignInMethod::Password));
    let servers = drawn(&imap, None, true);
    assert_eq!((servers.line, servers.password), (Password, None));

    // A JMAP account has no servers to edit, so its section takes it, a token as well.
    let jmap = entry(AccountKind::Jmap, SignInMethod::Password);
    let section = drawn(&jmap, None, true);
    assert_eq!(section.line, Refused);
    assert_eq!(
        section.password,
        field(AccountSecretKind::PasswordOrToken, true)
    );
    assert!(
        !section.switches,
        "replacing a password is not another way in"
    );
    assert_eq!(drawn(&jmap, None, false).password, None);
}

#[test]
fn a_calendar_account_has_a_section_only_at_a_nextcloud_server() {
    let mut dav = entry(AccountKind::Dav, SignInMethod::Password);
    assert_eq!(section(&dav, None, false, &Hold::default()), None);
    assert_eq!(
        section(
            &dav,
            Some(&options(false, false, false)),
            false,
            &Hold::default()
        ),
        None
    );

    let typed = drawn(&dav, Some(&options(false, false, true)), false);
    assert_eq!((typed.line, typed.routes), (Password, vec![ToNextcloud]));
    assert!(!typed.switches);

    dav.nextcloud = true;
    let made = drawn(&dav, None, false);
    assert_eq!(made.line, Nextcloud);
    assert!(made.routes.is_empty());
    assert_eq!(
        drawn(&dav, Some(&options(false, false, true)), false).routes,
        [SignInAgain]
    );
    assert_eq!(drawn(&dav, None, true).line, Refused);
}

#[test]
fn a_password_being_tried_runs_its_save_and_holds_the_rest() {
    let account = entry(AccountKind::Imap, SignInMethod::Provider);
    let answer = options(true, true, false);
    let hold = Hold {
        busy: true,
        own: vec![PageWork::Password],
    };
    let trying = section(&account, Some(&answer), false, &hold).expect("a section");
    assert_eq!(
        trying.password.map(|field| field.save),
        Some(ControlState {
            enabled: false,
            running: true,
        })
    );
    assert_eq!(
        trying.control,
        ControlState {
            enabled: false,
            running: false,
        }
    );

    let elsewhere = Hold {
        busy: true,
        own: Vec::new(),
    };
    let held = section(&account, Some(&answer), false, &elsewhere).expect("a section");
    assert!(!held.password.expect("a field").save.running);
}

#[test]
fn a_server_is_asked_once_and_an_answer_about_a_changed_sign_in_is_dropped() {
    let answers = SignInAnswers::default();
    let ticket = answers.ask("a", false).expect("asked");
    assert_eq!(answers.ask("a", true), None, "one ask at a time");
    answers.answered("a", ticket, options(true, false, false));
    assert_eq!(answers.options("a"), Some(options(true, false, false)));
    assert_eq!(answers.ask("a", false), None, "answered");

    // The page opened again: asked again, and the answer it had is drawn meanwhile.
    let reopened = answers.ask("a", true).expect("asked again");
    assert_eq!(answers.options("a"), Some(options(true, false, false)));
    answers.answered("a", reopened, options(false, true, false));
    assert_eq!(answers.options("a"), Some(options(false, true, false)));

    // The account's way in changed while it was asked: the answer is about the old one.
    answers.forget("a");
    let stale = answers.ask("a", false).expect("asked again");
    answers.forget("a");
    answers.answered("a", stale, options(false, true, false));
    assert_eq!(answers.options("a"), None);
}
