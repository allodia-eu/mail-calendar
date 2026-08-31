//! The Spanish message bodies of the showcase (screenshot) dataset — the twin of `super::en`,
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
        "p-report" => report_multipart(REPORT, "uso-junio.csv", REPORT_CSV),
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
<p>Hola Eva:</p>
<p>Dejo el arranque en la agenda de todos. El jueves por la tarde nos va bien a Tom y a mí &mdash; avísame si te choca con algo y lo muevo.</p>
<p>Sofía</p>
</div>"#;

const REPORT: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;"><h2 style="color:#16598D;margin:0 0 12px;">Tu informe de uso de junio</h2><p>Hola, Eva:</p><p>Gracias por usar Example Cloud. Tu informe de uso de junio va adjunto en formato CSV &mdash; ábrelo cuando quieras.</p><p>Un saludo,<br>El equipo de Example Cloud</p></div>"#;

const REPORT_CSV: &str = "métrica,valor\r\n\
                          Mensajes recibidos,1284\r\n\
                          Mensajes enviados,318\r\n\
                          Almacenamiento usado (GB),4.2\r\n";

const WELCOME: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;">
<h1 style="color:#16598D;font-size:22px;margin:0 0 14px;">Te damos la bienvenida a Allodia Mail &amp; Calendar</h1>
<p>Hola, Eva:</p>
<p>Ya está todo listo. Allodia Mail &amp; Calendar es un cliente <strong>soberano</strong> para el correo y el calendario que ya tienes: tus mensajes se quedan en tu propio proveedor, nunca con nosotros, y no hay ninguna nube estadounidense de por medio.</p>
<p style="margin:18px 0 6px;font-weight:600;">Algunas cosas que puedes probar:</p>
<ul style="margin:0 0 14px;padding-left:20px;">
<li>Conecta otra cuenta: todo se reúne en una única bandeja de entrada.</li>
<li>Elige <em>en cada cuenta</em>, desde los ajustes, hasta dónde quieres sincronizar.</li>
<li>Las imágenes remotas se bloquean por omisión, así los remitentes no ven cuándo lees.</li>
</ul>
<p>Bienvenida a bordo,<br>El equipo de Allodia</p>
</div>"#;

const LAUNCH_1: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Hola, Eva:</p>
<p>¿Puedes echar un último vistazo a la lista de comprobación del lanzamiento antes de cerrar el jueves? Sobre todo me interesa tu visto bueno al plan de reversión.</p>
<p>Por lo demás, por nuestra parte está todo en verde.</p>
<p>Gracias,<br>Tom</p>
</div>"#;

const LAUNCH_2: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Hola, Tom:</p>
<p>La lista de comprobación tiene buena pinta. Un ajuste en el plan de reversión &mdash; mantengamos la versión anterior lista durante 24 horas en lugar de 6 &mdash; y, por mi parte, adelante.</p>
<p>Eva</p>
</div>"#;

const LAUNCH_3: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Perfecto, gracias por responder tan rápido. El jueves salimos. Aviso al equipo y actualizo el guion con la ventana de 24 horas.</p>
<p>Tom</p>
</div>"#;

const CONTRACT: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Estimada Eva:</p>
<p>La versión definitiva del acuerdo de colaboración está lista para tu firma. No ha cambiado nada desde la última revisión, salvo la fecha de entrada en vigor.</p>
<p>Dinos si hay que cambiar algo más.</p>
<p>Un cordial saludo,<br>Northwind Legal</p>
</div>"#;

const NEWSLETTER: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;max-width:640px;">
<img src="https://cdn.europeandigital.example/header.png" width="640" alt="European Digital Weekly" style="max-width:100%;border-radius:12px;">
<h1 style="color:#16598D;font-size:20px;margin:16px 0 10px;">Esta semana en la tecnología europea</h1>
<p>Las noticias para quienes construyen y compran, y quieren saber dónde están sus datos.</p>
<h2 style="color:#16598D;font-size:16px;margin:18px 0 6px;">La soberanía avanza un paso más</h2>
<p>Unas nuevas directrices aclaran qué debe significar de verdad &laquo;alojado en la UE&raquo; &mdash; y por qué la región, por sí sola, no es jurisdicción.</p>
<h2 style="color:#16598D;font-size:16px;margin:18px 0 6px;">Tres herramientas que seguimos</h2>
<ol style="margin:0 0 14px;padding-left:20px;">
<li>Una sincronización de calendario autoalojable que puedes controlar de verdad.</li>
<li>Una pasarela de modelos gestionada en la UE, con enrutado por clave.</li>
<li>Un almacén de documentos pequeño y rápido, basado en estándares abiertos.</li>
</ol>
<p style="color:#5F6B73;font-size:12px;">Recibes este mensaje porque te suscribiste. Gestiona tus preferencias o date de baja.</p>
</div>"#;

const WORK_WELCOME: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.55;">
<h2 style="color:#16598D;margin:0 0 12px;">Bienvenida a tu primera semana</h2>
<p>Hola, Eva:</p>
<p>¡Nos alegra tenerte aquí! Todo lo que necesitas para empezar con buen pie está en el espacio de incorporación, y Sofia, tu mentora, se pondrá hoy en contacto contigo.</p>
<p>Nos vemos en la reunión diaria,<br>El equipo de Northwind People</p>
</div>"#;

const WORK_2FA: &str = r#"<div style="font-family:Segoe UI,Segoe,sans-serif;color:#0F3A5C;line-height:1.5;">
<p>Hola, Eva:</p>
<p>Activa el inicio de sesión en dos pasos antes del viernes para mantener tu cuenta protegida. Se tarda unos dos minutos.</p>
<p>Gracias,<br>Northwind IT</p>
</div>"#;
