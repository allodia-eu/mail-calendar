//! How a browser sign-in is drawn on its account's page: on the Sign-in section for the
//! account's own, on the separate server's section for that server's.

use crate::{
    AccountCapability, AccountPage, BrowserSignIn, ControlState, MailcalApp,
    account_page::browser::{Out, Target},
};

impl MailcalApp {
    /// Draws `account`'s sign-in on its page: the control that started it running while the
    /// server starts or completes it, the password field held while it is out, and the section
    /// saying it waits on the browser.
    pub(crate) fn draw_browser_sign_in(&self, page: &mut AccountPage) {
        let Some(Out {
            record: browser,
            target,
            adding,
            working,
        }) = self.account_work.browser.of(&page.account)
        else {
            return;
        };
        if let Target::Separate { uses } = target {
            return draw_separate(page, &uses, browser, working);
        }
        let running = ControlState {
            enabled: false,
            running: true,
        };
        if working {
            for switch in &mut page.uses {
                if adding.contains(&switch.capability) {
                    switch.control = running;
                }
            }
        }
        if let Some(section) = &mut page.sign_in {
            if working && adding.is_empty() {
                section.control = running;
            }
            if let Some(field) = &mut section.password {
                field.save.enabled = false;
            }
            section.browser = Some(browser);
        }
    }
}

/// Draws a separate server's sign-in on `page`'s section for `uses`: its control running while
/// `working`, Connect held while it is out, and the section saying it waits on the browser.
fn draw_separate(
    page: &mut AccountPage,
    uses: &[AccountCapability],
    browser: BrowserSignIn,
    working: bool,
) {
    let Some(section) = page
        .separate_server
        .as_mut()
        .filter(|section| section.uses == uses)
    else {
        return;
    };
    if working {
        section.sign_in_control = ControlState {
            enabled: false,
            running: true,
        };
    }
    section.connect.enabled = false;
    section.stop_using.enabled = false;
    section.browser = Some(browser);
}
