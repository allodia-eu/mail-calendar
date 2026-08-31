//! The English message bodies of the showcase (screenshot) dataset — see [`super`]. Its Dutch
//! twin is `super::nl`, keyed identically so both locales render the same messages.

use super::{html, report_multipart};

pub(super) fn body(key: &str) -> Option<Vec<u8>> {
    let mime = match key {
        "p-welcome" => html(WELCOME),
        "p-launch-1" => html(LAUNCH_1),
        "p-launch-2" => html(LAUNCH_2),
        "p-launch-3" => html(LAUNCH_3),
        "p-contract" => html(CONTRACT),
        "p-newsletter" => html(NEWSLETTER),
        "p-report" => report_multipart(REPORT, "june-report.csv", REPORT_CSV),
        "w-welcome" => html(WORK_WELCOME),
        "w-2fa" => html(WORK_2FA),
        _ => return None,
    };
    Some(mime)
}

/// The invitation mail's readable half. Deliberately short: the card the core builds above it is
/// what the screenshot is of, and a long body would push the Accept / Maybe / Decline row and the
/// day preview off screen. Keyed by `super::body`, not by the match above — every locale's
/// invitation is assembled from one place.
pub(super) const INVITE: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Hi Eva,</p>
<p>Putting the kickoff in everyone's diary. Thursday afternoon suits Tom and me &mdash; shout if it clashes with something and I'll move it.</p>
<p>Sofia</p>
</div>"#;

const REPORT: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;"><h2 style="color:#16598D;margin:0 0 12px;">Your June usage report</h2><p>Hi Eva,</p><p>Thanks for using Example Cloud. Your usage summary for June is attached as a CSV &mdash; open it any time.</p><p>Warm regards,<br>The Example Cloud team</p></div>"#;

const REPORT_CSV: &str = "metric,value\r\n\
                          Messages received,1284\r\n\
                          Messages sent,318\r\n\
                          Storage used (GB),4.2\r\n";

const WELCOME: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;">
<h1 style="color:#16598D;font-size:22px;margin:0 0 14px;">Welcome to Allodia Mail &amp; Calendar</h1>
<p>Hi Eva,</p>
<p>You're all set. Allodia Mail &amp; Calendar is a <strong>sovereign</strong> client over the mail and calendar you already own &mdash; your messages stay with your own provider, never with us, and there's no US-cloud in the middle.</p>
<p style="margin:18px 0 6px;font-weight:600;">A few things worth trying:</p>
<ul style="margin:0 0 14px;padding-left:20px;">
<li>Connect another account &mdash; everything lands in one unified inbox.</li>
<li>Choose how far back to sync, <em>per account</em>, under Settings.</li>
<li>Remote images are blocked by default, so senders can't track when you read.</li>
</ul>
<p>Welcome aboard,<br>The Allodia team</p>
</div>"#;

const LAUNCH_1: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Hi Eva,</p>
<p>Can you take a last look at the launch checklist before we lock Thursday? I'd like your sign-off on the rollback plan in particular.</p>
<p>Everything else is green on our side.</p>
<p>Thanks,<br>Tom</p>
</div>"#;

const LAUNCH_2: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Hi Tom,</p>
<p>Checklist looks solid. One tweak to the rollback plan &mdash; let's keep the previous build warm for 24h rather than 6 &mdash; and then it's a go from me.</p>
<p>Eva</p>
</div>"#;

const LAUNCH_3: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Perfect &mdash; thanks for the quick turnaround. We're go for Thursday. I'll let the team know and update the runbook with the 24h window.</p>
<p>Tom</p>
</div>"#;

const CONTRACT: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Dear Eva,</p>
<p>The final version of the partnership agreement is ready for your signature. Nothing has changed since the last review except the effective date.</p>
<p>Please let me know if anything still needs adjusting.</p>
<p>Kind regards,<br>Northwind Legal</p>
</div>"#;

const NEWSLETTER: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;max-width:640px;">
<img src="https://cdn.europeandigital.example/header.png" width="640" alt="European Digital Weekly" style="max-width:100%;border-radius:12px;">
<h1 style="color:#16598D;font-size:20px;margin:16px 0 10px;">This week in European tech</h1>
<p>The headlines for builders and buyers who care about where their data lives.</p>
<h2 style="color:#16598D;font-size:16px;margin:18px 0 6px;">Sovereignty moves forward</h2>
<p>New guidance clarifies what "EU-hosted" really has to mean &mdash; and why region alone isn't jurisdiction.</p>
<h2 style="color:#16598D;font-size:16px;margin:18px 0 6px;">Three tools we're watching</h2>
<ol style="margin:0 0 14px;padding-left:20px;">
<li>A self-hostable calendar sync you can actually audit.</li>
<li>An EU-run model gateway with per-key routing.</li>
<li>A tiny, fast document store built on open standards.</li>
</ol>
<p style="color:#5F6B73;font-size:12px;">You're receiving this because you subscribed. Manage preferences or unsubscribe.</p>
</div>"#;

const WORK_WELCOME: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;">
<h2 style="color:#16598D;margin:0 0 12px;">Welcome to your first week</h2>
<p>Hi Eva,</p>
<p>We're glad you're here! Everything you need for a smooth start is in the onboarding space, and your buddy Sofia will reach out today.</p>
<p>See you at the team standup,<br>Northwind People team</p>
</div>"#;

const WORK_2FA: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Hi Eva,</p>
<p>To keep your account secure, please enable two-step sign-in before Friday. It takes about two minutes.</p>
<p>Thanks,<br>Northwind IT</p>
</div>"#;
