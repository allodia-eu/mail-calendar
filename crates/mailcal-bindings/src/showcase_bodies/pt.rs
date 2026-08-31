//! The Portuguese message bodies of the showcase (screenshot) dataset — the twin of
//! `super::en`, keyed identically so every locale renders the same messages: the same
//! attachment on the usage report, the same remote image on the newsletter. European
//! Portuguese (pt-PT), like the catalog.

use super::{html, report_multipart};

pub(super) fn body(key: &str) -> Option<Vec<u8>> {
    let mime = match key {
        "p-welcome" => html(WELCOME),
        "p-launch-1" => html(LAUNCH_1),
        "p-launch-2" => html(LAUNCH_2),
        "p-launch-3" => html(LAUNCH_3),
        "p-contract" => html(CONTRACT),
        "p-newsletter" => html(NEWSLETTER),
        "p-report" => report_multipart(REPORT, "utilizacao-junho.csv", REPORT_CSV),
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
<p>Olá Eva,</p>
<p>Vou deixar o arranque na agenda de todos. Quinta-feira à tarde dá jeito ao Tom e a mim &mdash; diga se colidir com algo e eu mudo.</p>
<p>Sofia</p>
</div>"#;

const REPORT: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;"><h2 style="color:#16598D;margin:0 0 12px;">O seu relatório de utilização de junho</h2><p>Olá Eva,</p><p>Obrigado por usar a Example Cloud. O seu relatório de utilização de junho segue em anexo, em CSV &mdash; abra-o quando quiser.</p><p>Com os melhores cumprimentos,<br>A equipa da Example Cloud</p></div>"#;

const REPORT_CSV: &str = "métrica,valor\r\n\
                          Mensagens recebidas,1284\r\n\
                          Mensagens enviadas,318\r\n\
                          Armazenamento usado (GB),4.2\r\n";

const WELCOME: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;">
<h1 style="color:#16598D;font-size:22px;margin:0 0 14px;">Bem-vinda ao Allodia Mail &amp; Calendar</h1>
<p>Olá Eva,</p>
<p>Está tudo pronto. O Allodia Mail &amp; Calendar é um cliente <strong>soberano</strong> para o correio e o calendário que já tem &mdash; as suas mensagens ficam no seu próprio fornecedor, nunca connosco, e não há nenhuma nuvem norte-americana pelo meio.</p>
<p style="margin:18px 0 6px;font-weight:600;">Algumas coisas para experimentar:</p>
<ul style="margin:0 0 14px;padding-left:20px;">
<li>Ligue outra conta &mdash; tudo se junta numa única caixa de entrada.</li>
<li>Escolha <em>em cada conta</em>, nas definições, até onde quer sincronizar.</li>
<li>As imagens remotas são bloqueadas por predefinição, para que os remetentes não vejam quando lê.</li>
</ul>
<p>Bem-vinda a bordo,<br>A equipa da Allodia</p>
</div>"#;

const LAUNCH_1: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Olá Eva,</p>
<p>Podes dar uma última vista de olhos à lista de verificação do lançamento antes de fecharmos a quinta-feira? Queria sobretudo o teu aval ao plano de reversão.</p>
<p>De resto, do nosso lado está tudo verde.</p>
<p>Obrigado,<br>Tom</p>
</div>"#;

const LAUNCH_2: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Olá Tom,</p>
<p>A lista de verificação está sólida. Um ajuste ao plano de reversão &mdash; mantenhamos a versão anterior pronta durante 24 horas em vez de 6 &mdash; e, da minha parte, podemos avançar.</p>
<p>Eva</p>
</div>"#;

const LAUNCH_3: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Perfeito &mdash; obrigado pela resposta rápida. Avançamos na quinta-feira. Eu aviso a equipa e atualizo o guião com a janela de 24 horas.</p>
<p>Tom</p>
</div>"#;

const CONTRACT: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Exma. Sra. Eva Jansen,</p>
<p>A versão final do acordo de parceria está pronta para a sua assinatura. Nada mudou desde a última revisão, exceto a data de entrada em vigor.</p>
<p>Diga-nos se houver algo a alterar.</p>
<p>Com os melhores cumprimentos,<br>Northwind Legal</p>
</div>"#;

const NEWSLETTER: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;max-width:640px;">
<img src="https://cdn.europeandigital.example/header.png" width="640" alt="European Digital Weekly" style="max-width:100%;border-radius:12px;">
<h1 style="color:#16598D;font-size:20px;margin:16px 0 10px;">Esta semana na tecnologia europeia</h1>
<p>As notícias para quem constrói e para quem compra, e quer saber onde estão os seus dados.</p>
<h2 style="color:#16598D;font-size:16px;margin:18px 0 6px;">A soberania dá mais um passo</h2>
<p>Novas orientações esclarecem o que &laquo;alojado na UE&raquo; tem mesmo de significar &mdash; e porque é que a região, por si só, não é jurisdição.</p>
<h2 style="color:#16598D;font-size:16px;margin:18px 0 6px;">Três ferramentas que estamos a seguir</h2>
<ol style="margin:0 0 14px;padding-left:20px;">
<li>Uma sincronização de calendário autoalojada que pode mesmo controlar.</li>
<li>Um gateway de modelos operado na UE, com encaminhamento por chave.</li>
<li>Um armazenamento de documentos pequeno e rápido, assente em normas abertas.</li>
</ol>
<p style="color:#5F6B73;font-size:12px;">Recebe esta mensagem porque se inscreveu. Faça a gestão das preferências ou anule a subscrição.</p>
</div>"#;

const WORK_WELCOME: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;">
<h2 style="color:#16598D;margin:0 0 12px;">Bem-vinda à sua primeira semana</h2>
<p>Olá Eva,</p>
<p>Ainda bem que está connosco! Tudo o que precisa para começar bem está no espaço de integração, e a Sofia, a sua madrinha de acolhimento, entra hoje em contacto consigo.</p>
<p>Até à reunião diária da equipa,<br>A equipa da Northwind People</p>
</div>"#;

const WORK_2FA: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Olá Eva,</p>
<p>Ative o início de sessão em dois passos antes de sexta-feira para manter a sua conta segura. Demora cerca de dois minutos.</p>
<p>Obrigado,<br>Northwind IT</p>
</div>"#;
