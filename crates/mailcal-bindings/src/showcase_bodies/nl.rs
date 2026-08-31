//! The Dutch message bodies of the showcase (screenshot) dataset — the twin of `super::en`,
//! keyed identically so both locales render the same messages: the same attachment on the
//! usage report, the same remote image on the newsletter.

use super::{html, report_multipart};

pub(super) fn body(key: &str) -> Option<Vec<u8>> {
    let mime = match key {
        "p-welcome" => html(WELCOME),
        "p-launch-1" => html(LAUNCH_1),
        "p-launch-2" => html(LAUNCH_2),
        "p-launch-3" => html(LAUNCH_3),
        "p-contract" => html(CONTRACT),
        "p-newsletter" => html(NEWSLETTER),
        "p-report" => report_multipart(REPORT, "verbruik-juni.csv", REPORT_CSV),
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
<p>Hoi Eva,</p>
<p>Ik zet de kick-off vast in ieders agenda. Donderdagmiddag komt Tom en mij goed uit &mdash; laat het weten als het botst, dan verzet ik hem.</p>
<p>Sofia</p>
</div>"#;

const REPORT: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;"><h2 style="color:#16598D;margin:0 0 12px;">Je verbruiksoverzicht van juni</h2><p>Hallo Eva,</p><p>Bedankt dat je Example Cloud gebruikt. Je verbruiksoverzicht van juni zit als CSV in de bijlage &mdash; open het wanneer je wilt.</p><p>Hartelijke groet,<br>Het team van Example Cloud</p></div>"#;

const REPORT_CSV: &str = "meetwaarde,waarde\r\n\
                          Ontvangen berichten,1284\r\n\
                          Verzonden berichten,318\r\n\
                          Gebruikte opslag (GB),4.2\r\n";

const WELCOME: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;">
<h1 style="color:#16598D;font-size:22px;margin:0 0 14px;">Welkom bij Allodia Mail &amp; Calendar</h1>
<p>Hallo Eva,</p>
<p>Je bent klaar om te beginnen. Allodia Mail &amp; Calendar is een <strong>soevereine</strong> client op de mail en agenda die je al hebt &mdash; je berichten blijven bij je eigen provider, nooit bij ons, en er zit geen Amerikaanse cloud tussen.</p>
<p style="margin:18px 0 6px;font-weight:600;">Een paar dingen om te proberen:</p>
<ul style="margin:0 0 14px;padding-left:20px;">
<li>Koppel nog een account &mdash; alles komt samen in één postvak IN.</li>
<li>Kies <em>per account</em> hoe ver terug je synchroniseert, bij Instellingen.</li>
<li>Externe afbeeldingen worden standaard geblokkeerd, zodat afzenders niet kunnen zien wanneer je leest.</li>
</ul>
<p>Welkom aan boord,<br>Het team van Allodia</p>
</div>"#;

const LAUNCH_1: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Hallo Eva,</p>
<p>Kun je nog één keer naar de checklist voor de lancering kijken voordat we donderdag vastleggen? Ik wil vooral je akkoord op het terugrolplan.</p>
<p>Verder staat aan onze kant alles op groen.</p>
<p>Bedankt,<br>Tom</p>
</div>"#;

const LAUNCH_2: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Hallo Tom,</p>
<p>De checklist ziet er goed uit. Eén aanpassing aan het terugrolplan &mdash; laten we de vorige build 24 uur warm houden in plaats van 6 &mdash; en dan is het wat mij betreft een go.</p>
<p>Eva</p>
</div>"#;

const LAUNCH_3: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Perfect &mdash; bedankt voor de snelle reactie. We gaan donderdag live. Ik laat het het team weten en werk het draaiboek bij met het venster van 24 uur.</p>
<p>Tom</p>
</div>"#;

const CONTRACT: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Beste Eva,</p>
<p>De definitieve versie van de samenwerkingsovereenkomst ligt klaar voor je handtekening. Er is niets gewijzigd sinds de vorige review, behalve de ingangsdatum.</p>
<p>Laat het me weten als er nog iets moet worden aangepast.</p>
<p>Met vriendelijke groet,<br>Northwind Legal</p>
</div>"#;

const NEWSLETTER: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;max-width:640px;">
<img src="https://cdn.europeandigital.example/header.png" width="640" alt="European Digital Weekly" style="max-width:100%;border-radius:12px;">
<h1 style="color:#16598D;font-size:20px;margin:16px 0 10px;">Deze week in de Europese tech</h1>
<p>Het nieuws voor bouwers en inkopers die willen weten waar hun data staat.</p>
<h2 style="color:#16598D;font-size:16px;margin:18px 0 6px;">Soevereiniteit gaat een stap verder</h2>
<p>Nieuwe richtlijnen verduidelijken wat "in de EU gehost" echt moet betekenen &mdash; en waarom regio alleen nog geen jurisdictie is.</p>
<h2 style="color:#16598D;font-size:16px;margin:18px 0 6px;">Drie tools die we volgen</h2>
<ol style="margin:0 0 14px;padding-left:20px;">
<li>Een zelf te hosten agendasynchronisatie die je echt kunt controleren.</li>
<li>Een in de EU beheerde modelgateway met routering per sleutel.</li>
<li>Een kleine, snelle documentopslag op open standaarden.</li>
</ol>
<p style="color:#5F6B73;font-size:12px;">Je ontvangt dit omdat je je hebt aangemeld. Beheer je voorkeuren of meld je af.</p>
</div>"#;

const WORK_WELCOME: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;">
<h2 style="color:#16598D;margin:0 0 12px;">Welkom in je eerste week</h2>
<p>Hallo Eva,</p>
<p>Fijn dat je er bent! Alles voor een vliegende start staat klaar in de onboardingomgeving, en je buddy Sofia neemt vandaag contact met je op.</p>
<p>Tot bij de teamstandup,<br>Het team van Northwind People</p>
</div>"#;

const WORK_2FA: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Hallo Eva,</p>
<p>Zet vóór vrijdag aanmelding in twee stappen aan om je account veilig te houden. Het kost ongeveer twee minuten.</p>
<p>Bedankt,<br>Northwind IT</p>
</div>"#;
