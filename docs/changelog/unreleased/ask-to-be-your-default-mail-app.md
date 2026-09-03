# Offer to become your default mail app

Platforms: macos, windows
Bump: minor

> The offer is put **once**, and the core decides when: never before an account exists, never when
> the app is already the handler, and never twice. A dismissed prompt counts as answered, because
> an unanswered question is not permission to ask again, and Settings → General is the way back.
> No client keeps its own "have we asked?" flag, so nothing can disagree with what Settings shows.
>
> What the platforms can actually do differs more than expected. macOS shows a system consent
> alert, but only in the Developer ID build: the App Sandbox refuses `setDefaultApplication`, so
> the Mac App Store build reports `unsupported` at runtime (read from the sandbox container
> variable) and shows neither the offer nor the row. Windows has had no API to set a handler since
> Windows 10 by design, so it deep-links its own Default apps page. iOS needs the
> `com.apple.developer.mail-client` entitlement, which Apple grants by request; until then it
> reports `unsupported` too, because sending someone to a list this app is not in is worse than
> silence. Linux and Android can be asked nothing at all.

**English**

```
Allodia Mail & Calendar can now offer to become your default mail app, so mail links open here.
Change your mind any time in Settings → General.
```

**Nederlands**

```
Allodia Mail & Calendar kan nu aanbieden om je standaard e-mailapp te worden, zodat
e-mailkoppelingen hier openen. Je wijzigt dit wanneer je wilt in Instellingen → Algemeen.
```

**Deutsch**

```
Allodia Mail & Calendar kann sich jetzt als Ihre Standard-E-Mail-App anbieten, damit E-Mail-Links
hier geöffnet werden. Sie ändern das jederzeit unter Einstellungen → Allgemein.
```

**Français**

```
Allodia Mail & Calendar peut désormais proposer de devenir votre application de messagerie par
défaut, pour que les liens e-mail s'ouvrent ici. Modifiable à tout moment dans Réglages → Général.
```

**Español**

```
Allodia Mail & Calendar ahora puede ofrecerse como tu aplicación de correo predeterminada, para que
los enlaces de correo se abran aquí. Cámbialo cuando quieras en Ajustes → General.
```

**Italiano**

```
Allodia Mail & Calendar può ora proporsi come app di posta predefinita, così i link e-mail si
aprono qui. Modificabile in qualsiasi momento in Impostazioni → Generali.
```

**Português**

```
O Allodia Mail & Calendar pode agora propor-se como a sua aplicação de e-mail predefinida, para que
as ligações de e-mail abram aqui. Altere quando quiser em Definições → Geral.
```
