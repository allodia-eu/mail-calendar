//! The German message bodies of the showcase (screenshot) dataset — the twin of `super::en`,
//! keyed identically so every locale renders the same messages: the same attachment on the
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
        "p-report" => report_multipart(REPORT, "nutzung-juni.csv", REPORT_CSV),
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
<p>Hallo Eva,</p>
<p>ich trage den Kick-off allen in den Kalender ein. Donnerstagnachmittag passt Tom und mir &mdash; sagen Sie Bescheid, falls es kollidiert, dann verschiebe ich ihn.</p>
<p>Sofia</p>
</div>"#;

const REPORT: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;"><h2 style="color:#16598D;margin:0 0 12px;">Ihre Nutzungsübersicht für Juni</h2><p>Hallo Eva,</p><p>danke, dass Sie Example Cloud nutzen. Ihre Nutzungsübersicht für Juni liegt als CSV bei &mdash; öffnen Sie sie, wann immer Sie möchten.</p><p>Herzliche Grüße,<br>Ihr Team von Example Cloud</p></div>"#;

const REPORT_CSV: &str = "kennzahl,wert\r\n\
                          Empfangene Nachrichten,1284\r\n\
                          Gesendete Nachrichten,318\r\n\
                          Belegter Speicher (GB),4.2\r\n";

const WELCOME: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;">
<h1 style="color:#16598D;font-size:22px;margin:0 0 14px;">Willkommen bei Allodia Mail &amp; Calendar</h1>
<p>Hallo Eva,</p>
<p>alles ist bereit. Allodia Mail &amp; Calendar ist ein <strong>souveräner</strong> Client für die Mail und den Kalender, die Sie bereits haben &mdash; Ihre Nachrichten bleiben bei Ihrem eigenen Anbieter, niemals bei uns, und keine US-Cloud sitzt dazwischen.</p>
<p style="margin:18px 0 6px;font-weight:600;">Ein paar Dinge zum Ausprobieren:</p>
<ul style="margin:0 0 14px;padding-left:20px;">
<li>Verbinden Sie ein weiteres Konto &mdash; alles läuft in einem Posteingang zusammen.</li>
<li>Wählen Sie <em>pro Konto</em> in den Einstellungen, wie weit zurück synchronisiert wird.</li>
<li>Externe Bilder werden standardmäßig blockiert, damit Absender nicht sehen, wann Sie lesen.</li>
</ul>
<p>Willkommen an Bord,<br>Ihr Team von Allodia</p>
</div>"#;

const LAUNCH_1: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Hallo Eva,</p>
<p>kannst du dir die Checkliste für den Launch noch einmal ansehen, bevor wir den Donnerstag festzurren? Vor allem hätte ich gern dein Okay zum Rollback-Plan.</p>
<p>Ansonsten steht bei uns alles auf Grün.</p>
<p>Danke,<br>Tom</p>
</div>"#;

const LAUNCH_2: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Hallo Tom,</p>
<p>die Checkliste sieht solide aus. Eine Anpassung am Rollback-Plan &mdash; lass uns den vorherigen Build 24 Stunden warmhalten statt 6 &mdash; und dann ist es von mir aus ein Go.</p>
<p>Eva</p>
</div>"#;

const LAUNCH_3: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Perfekt &mdash; danke für die schnelle Rückmeldung. Wir gehen am Donnerstag live. Ich sage dem Team Bescheid und aktualisiere den Ablaufplan mit dem 24-Stunden-Fenster.</p>
<p>Tom</p>
</div>"#;

const CONTRACT: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Sehr geehrte Frau Jansen,</p>
<p>die endgültige Fassung des Partnerschaftsvertrags liegt zu Ihrer Unterschrift bereit. Gegenüber der letzten Durchsicht hat sich nichts geändert außer dem Datum des Inkrafttretens.</p>
<p>Sagen Sie uns Bescheid, falls noch etwas angepasst werden muss.</p>
<p>Mit freundlichen Grüßen,<br>Northwind Legal</p>
</div>"#;

const NEWSLETTER: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;max-width:640px;">
<img src="https://cdn.europeandigital.example/header.png" width="640" alt="European Digital Weekly" style="max-width:100%;border-radius:12px;">
<h1 style="color:#16598D;font-size:20px;margin:16px 0 10px;">Diese Woche in der europäischen Tech-Welt</h1>
<p>Die Nachrichten für alle, die bauen und einkaufen &mdash; und wissen wollen, wo ihre Daten liegen.</p>
<h2 style="color:#16598D;font-size:16px;margin:18px 0 6px;">Souveränität geht einen Schritt weiter</h2>
<p>Neue Leitlinien stellen klar, was &bdquo;in der EU gehostet&ldquo; wirklich bedeuten muss &mdash; und warum die Region allein noch keine Rechtshoheit ist.</p>
<h2 style="color:#16598D;font-size:16px;margin:18px 0 6px;">Drei Tools, die wir beobachten</h2>
<ol style="margin:0 0 14px;padding-left:20px;">
<li>Eine selbst hostbare Kalendersynchronisation, die Sie wirklich kontrollieren können.</li>
<li>Ein in der EU betriebenes Modell-Gateway mit Routing pro Schlüssel.</li>
<li>Ein kleiner, schneller Dokumentspeicher auf offenen Standards.</li>
</ol>
<p style="color:#5F6B73;font-size:12px;">Sie erhalten diese E-Mail, weil Sie sich angemeldet haben. Einstellungen verwalten oder abmelden.</p>
</div>"#;

const WORK_WELCOME: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;">
<h2 style="color:#16598D;margin:0 0 12px;">Willkommen in Ihrer ersten Woche</h2>
<p>Hallo Eva,</p>
<p>schön, dass Sie da sind! Alles für einen guten Start finden Sie im Onboarding-Bereich, und Ihre Buddy Sofia meldet sich heute bei Ihnen.</p>
<p>Bis zum Team-Standup,<br>Ihr Team von Northwind People</p>
</div>"#;

const WORK_2FA: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Hallo Eva,</p>
<p>bitte aktivieren Sie vor Freitag die Anmeldung in zwei Schritten, damit Ihr Konto sicher bleibt. Es dauert etwa zwei Minuten.</p>
<p>Danke,<br>Northwind IT</p>
</div>"#;
