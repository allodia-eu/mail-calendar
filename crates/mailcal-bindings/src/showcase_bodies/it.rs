//! The Italian message bodies of the showcase (screenshot) dataset — the twin of `super::en`,
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
        "p-report" => report_multipart(REPORT, "utilizzo-giugno.csv", REPORT_CSV),
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
<p>Ciao Eva,</p>
<p>Metto l’avvio in agenda a tutti. Giovedì pomeriggio va bene a me e a Tom &mdash; dimmi se si sovrappone a qualcosa e lo sposto.</p>
<p>Sofia</p>
</div>"#;

const REPORT: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;"><h2 style="color:#16598D;margin:0 0 12px;">Il tuo report di utilizzo di giugno</h2><p>Ciao Eva,</p><p>grazie per aver scelto Example Cloud. Il tuo report di utilizzo di giugno è allegato in formato CSV &mdash; aprilo quando vuoi.</p><p>Un cordiale saluto,<br>Il team di Example Cloud</p></div>"#;

const REPORT_CSV: &str = "metrica,valore\r\n\
                          Messaggi ricevuti,1284\r\n\
                          Messaggi inviati,318\r\n\
                          Spazio utilizzato (GB),4.2\r\n";

const WELCOME: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;">
<h1 style="color:#16598D;font-size:22px;margin:0 0 14px;">Ti diamo il benvenuto in Allodia Mail &amp; Calendar</h1>
<p>Ciao Eva,</p>
<p>è tutto pronto. Allodia Mail &amp; Calendar è un client <strong>sovrano</strong> per la posta e il calendario che già usi: i tuoi messaggi restano presso il tuo provider, mai da noi, e nessun cloud statunitense si mette in mezzo.</p>
<p style="margin:18px 0 6px;font-weight:600;">Qualche cosa da provare:</p>
<ul style="margin:0 0 14px;padding-left:20px;">
<li>Collega un altro account: tutto confluisce in un&rsquo;unica casella di posta in arrivo.</li>
<li>Scegli <em>per ogni account</em>, nelle impostazioni, quanto indietro sincronizzare.</li>
<li>Le immagini remote sono bloccate per impostazione predefinita, così i mittenti non vedono quando leggi.</li>
</ul>
<p>Benvenuta a bordo,<br>Il team di Allodia</p>
</div>"#;

const LAUNCH_1: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Ciao Eva,</p>
<p>puoi dare un&rsquo;ultima occhiata alla checklist del lancio prima che fissiamo giovedì? Mi interessa soprattutto il tuo via libera sul piano di rollback.</p>
<p>Per il resto, da parte nostra è tutto verde.</p>
<p>Grazie,<br>Tom</p>
</div>"#;

const LAUNCH_2: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Ciao Tom,</p>
<p>la checklist regge. Una modifica al piano di rollback &mdash; teniamo la build precedente pronta per 24 ore invece di 6 &mdash; e per me si può partire.</p>
<p>Eva</p>
</div>"#;

const LAUNCH_3: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Perfetto &mdash; grazie per la risposta rapida. Giovedì si parte. Avviso il team e aggiorno la scaletta con la finestra di 24 ore.</p>
<p>Tom</p>
</div>"#;

const CONTRACT: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Gentile Eva,</p>
<p>la versione definitiva dell&rsquo;accordo di partnership è pronta per la tua firma. Rispetto all&rsquo;ultima revisione non è cambiato nulla, tranne la data di decorrenza.</p>
<p>Facci sapere se c&rsquo;è ancora qualcosa da modificare.</p>
<p>Cordiali saluti,<br>Northwind Legal</p>
</div>"#;

const NEWSLETTER: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;max-width:640px;">
<img src="https://cdn.europeandigital.example/header.png" width="640" alt="European Digital Weekly" style="max-width:100%;border-radius:12px;">
<h1 style="color:#16598D;font-size:20px;margin:16px 0 10px;">Questa settimana nel tech europeo</h1>
<p>Le notizie per chi costruisce e per chi acquista, e vuole sapere dove sono i propri dati.</p>
<h2 style="color:#16598D;font-size:16px;margin:18px 0 6px;">La sovranità fa un passo avanti</h2>
<p>Nuove linee guida chiariscono che cosa debba davvero significare &laquo;ospitato nell&rsquo;UE&raquo; &mdash; e perché la regione, da sola, non è giurisdizione.</p>
<h2 style="color:#16598D;font-size:16px;margin:18px 0 6px;">Tre strumenti che stiamo seguendo</h2>
<ol style="margin:0 0 14px;padding-left:20px;">
<li>Una sincronizzazione del calendario self-hosted che puoi davvero controllare.</li>
<li>Un gateway di modelli gestito nell&rsquo;UE, con instradamento per chiave.</li>
<li>Un archivio documenti piccolo e veloce, basato su standard aperti.</li>
</ol>
<p style="color:#5F6B73;font-size:12px;">Ricevi questo messaggio perché ti sei iscritta. Gestisci le preferenze o annulla l&rsquo;iscrizione.</p>
</div>"#;

const WORK_WELCOME: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;">
<h2 style="color:#16598D;margin:0 0 12px;">Benvenuta nella tua prima settimana</h2>
<p>Ciao Eva,</p>
<p>siamo felici di averti con noi! Tutto ciò che ti serve per partire bene è nello spazio di onboarding, e Sofia, la tua buddy, ti contatta oggi.</p>
<p>Ci vediamo allo standup del team,<br>Il team di Northwind People</p>
</div>"#;

const WORK_2FA: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Ciao Eva,</p>
<p>attiva l&rsquo;accesso in due passaggi entro venerdì per mantenere sicuro il tuo account. Bastano circa due minuti.</p>
<p>Grazie,<br>Northwind IT</p>
</div>"#;
