# Mail links open the composer on Mac

Platforms: macos
Bump: minor

> Gate 12's last platform that can actually ship it. Nothing new was decided: the URI is decoded by
> `parse_mailto_uri`, so Apple inherits the same header allowlist the other three already honour,
> and `MailLinkRequest` adds only an id, for the reason `AgentDraftRequest` carries one. Without it
> `.fullScreenCover(item:)` compares the second tap of the same link equal to the first and silently
> does nothing.
>
> `onOpenURL` sits on the shell rather than the `WindowGroup` because the model is that view's own
> state. No scheme gate is needed, unlike the other three clients: every OAuth redirect is captured
> inside its own `ASWebAuthenticationSession` and never arrives as an opened URL.
>
> A link arriving before the first account is held on the model and opened once one exists, and one
> arriving over a draft goes through the discard guard, so a web page cannot throw away a
> half-written message.
>
> ⚠️ **iPhone and iPad are deliberately not claimed.** The same code and the same
> `CFBundleURLTypes` declaration ship there, but iOS routes a mail link to the app set as the
> *default* mail app and to nothing else, and being set as that needs
> `com.apple.developer.mail-client`, which Apple grants only by request. So `onOpenURL` never fires
> for a mail link on iOS yet, the capability matrix says 🚧, and this note says Mac alone. See
> `docs/os-integration.md` → Known gaps.

**English**

```
Click a mail link on your Mac and Allodia Mail & Calendar opens a message ready to write,
addressed for you.
```

**Nederlands**

```
Klik op een e-mailkoppeling op je Mac en Allodia Mail & Calendar opent een bericht dat klaarstaat
om te schrijven, met de geadresseerde er al in.
```

**Deutsch**

```
Klicken Sie auf Ihrem Mac auf einen E-Mail-Link, und Allodia Mail & Calendar öffnet eine Nachricht,
die bereits adressiert ist und auf Ihren Text wartet.
```

**Français**

```
Cliquez sur un lien e-mail sur votre Mac et Allodia Mail & Calendar ouvre un message prêt à écrire,
déjà adressé.
```

**Español**

```
Haz clic en un enlace de correo en tu Mac y Allodia Mail & Calendar abre un mensaje listo para
escribir, ya dirigido.
```

**Italiano**

```
Fai clic su un link e-mail sul tuo Mac e Allodia Mail & Calendar apre un messaggio pronto da
scrivere, già indirizzato.
```

**Português**

```
Clique num link de e-mail no seu Mac e o Allodia Mail & Calendar abre uma mensagem pronta a
escrever, já endereçada.
```
