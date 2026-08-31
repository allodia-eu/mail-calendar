//! The French message bodies of the showcase (screenshot) dataset — the twin of `super::en`,
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
        "p-report" => report_multipart(REPORT, "utilisation-juin.csv", REPORT_CSV),
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
<p>Bonjour Eva,</p>
<p>Je pose le lancement dans l’agenda de chacun. Jeudi après-midi arrange Tom et moi &mdash; dis-moi si cela tombe mal, je le déplacerai.</p>
<p>Sofia</p>
</div>"#;

const REPORT: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;"><h2 style="color:#16598D;margin:0 0 12px;">Votre rapport d&rsquo;utilisation de juin</h2><p>Bonjour Eva,</p><p>Merci d&rsquo;utiliser Example Cloud. Votre rapport d&rsquo;utilisation de juin est joint au format CSV &mdash; ouvrez-le quand vous le souhaitez.</p><p>Bien cordialement,<br>L&rsquo;équipe Example Cloud</p></div>"#;

const REPORT_CSV: &str = "indicateur,valeur\r\n\
                          Messages reçus,1284\r\n\
                          Messages envoyés,318\r\n\
                          Stockage utilisé (Go),4.2\r\n";

const WELCOME: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;">
<h1 style="color:#16598D;font-size:22px;margin:0 0 14px;">Bienvenue dans Allodia Mail &amp; Calendar</h1>
<p>Bonjour Eva,</p>
<p>Tout est prêt. Allodia Mail &amp; Calendar est un client <strong>souverain</strong> pour le courrier et le calendrier que vous avez déjà &mdash; vos messages restent chez votre propre fournisseur, jamais chez nous, et aucun cloud américain ne s&rsquo;intercale.</p>
<p style="margin:18px 0 6px;font-weight:600;">Quelques pistes pour commencer :</p>
<ul style="margin:0 0 14px;padding-left:20px;">
<li>Connectez un autre compte &mdash; tout se retrouve dans une seule boîte de réception.</li>
<li>Choisissez <em>pour chaque compte</em>, dans les réglages, jusqu&rsquo;où remonter la synchronisation.</li>
<li>Les images distantes sont bloquées par défaut : les expéditeurs ne voient pas quand vous lisez.</li>
</ul>
<p>Bienvenue à bord,<br>L&rsquo;équipe Allodia</p>
</div>"#;

const LAUNCH_1: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Salut Eva,</p>
<p>Peux-tu jeter un dernier œil à la checklist du lancement avant qu&rsquo;on fige le jeudi ? J&rsquo;aimerais surtout ton feu vert sur le plan de retour arrière.</p>
<p>Pour le reste, tout est au vert de notre côté.</p>
<p>Merci,<br>Tom</p>
</div>"#;

const LAUNCH_2: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Salut Tom,</p>
<p>La checklist tient la route. Un ajustement sur le plan de retour arrière &mdash; gardons la version précédente au chaud 24 heures plutôt que 6 &mdash; et pour moi, c&rsquo;est bon.</p>
<p>Eva</p>
</div>"#;

const LAUNCH_3: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Parfait &mdash; merci pour ta réactivité. C&rsquo;est parti pour jeudi. Je préviens l&rsquo;équipe et je mets à jour le déroulé avec la fenêtre de 24 heures.</p>
<p>Tom</p>
</div>"#;

const CONTRACT: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Bonjour Madame Jansen,</p>
<p>La version définitive du contrat de partenariat est prête pour votre signature. Rien n&rsquo;a changé depuis la dernière relecture, hormis la date d&rsquo;entrée en vigueur.</p>
<p>Dites-nous si quelque chose doit encore être modifié.</p>
<p>Cordialement,<br>Northwind Legal</p>
</div>"#;

const NEWSLETTER: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;max-width:640px;">
<img src="https://cdn.europeandigital.example/header.png" width="640" alt="European Digital Weekly" style="max-width:100%;border-radius:12px;">
<h1 style="color:#16598D;font-size:20px;margin:16px 0 10px;">Cette semaine dans la tech européenne</h1>
<p>L&rsquo;actualité de celles et ceux qui construisent et qui achètent &mdash; et qui veulent savoir où sont leurs données.</p>
<h2 style="color:#16598D;font-size:16px;margin:18px 0 6px;">La souveraineté avance</h2>
<p>De nouvelles lignes directrices précisent ce que &laquo;&nbsp;hébergé dans l&rsquo;UE&nbsp;&raquo; doit vraiment vouloir dire &mdash; et pourquoi la région ne fait pas la juridiction.</p>
<h2 style="color:#16598D;font-size:16px;margin:18px 0 6px;">Trois outils que nous suivons</h2>
<ol style="margin:0 0 14px;padding-left:20px;">
<li>Une synchronisation de calendrier auto-hébergeable que vous contrôlez vraiment.</li>
<li>Une passerelle de modèles opérée dans l&rsquo;UE, avec un routage par clé.</li>
<li>Un stockage de documents léger et rapide, bâti sur des standards ouverts.</li>
</ol>
<p style="color:#5F6B73;font-size:12px;">Vous recevez ce message car vous vous êtes inscrit. Gérer vos préférences ou vous désabonner.</p>
</div>"#;

const WORK_WELCOME: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;">
<h2 style="color:#16598D;margin:0 0 12px;">Bienvenue pour votre première semaine</h2>
<p>Bonjour Eva,</p>
<p>Nous sommes ravis de vous accueillir ! Tout ce qu&rsquo;il faut pour bien démarrer se trouve dans l&rsquo;espace d&rsquo;intégration, et Sofia, votre marraine, vous contacte aujourd&rsquo;hui.</p>
<p>À très vite au point d&rsquo;équipe,<br>L&rsquo;équipe Northwind People</p>
</div>"#;

const WORK_2FA: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Bonjour Eva,</p>
<p>Merci d&rsquo;activer la connexion en deux étapes avant vendredi afin de garder votre compte en sécurité. Cela prend environ deux minutes.</p>
<p>Merci,<br>Northwind IT</p>
</div>"#;
