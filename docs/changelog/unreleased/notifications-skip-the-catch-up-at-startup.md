# Notifications skip the catch-up a launch begins with

Platforms: linux
Bump: patch

> `collect_cached_new_mail` reported everything past the stored high-water mark, and a desktop
> launch opens with a catch-up sync, so starting the app after it had been closed for a while
> announced every message that had arrived meanwhile, each one already visible in the list behind
> the notifications. The core now floors a cached scan at the instant it was built: mail older than
> the session advances the mark and is never reported. The floor covers a whole session rather than
> one pass, which is why it could not be expressed as a mark, and it sits in the core rather than
> in each host, so every desktop inherits it. `run_background_sync` deliberately keeps no floor: a
> mobile pass exists to report exactly the mail that arrived while nobody was looking.

**English**

```
Starting the app no longer announces the mail that arrived while it was closed: notifications are for mail that arrives while you have it open.
```

**Nederlands**

```
Bij het starten meldt de app niet langer alles wat binnenkwam terwijl hij dicht was: meldingen gaan over post die binnenkomt terwijl hij openstaat.
```

**Deutsch**

```
Beim Start meldet die App nicht mehr alles, was eingegangen ist, während sie geschlossen war: Benachrichtigungen gelten für Post, die bei geöffneter App eintrifft.
```

**Français**

```
Au démarrage, l'application n'annonce plus les messages arrivés pendant qu'elle était fermée : les notifications concernent le courrier qui arrive pendant qu'elle est ouverte.
```

**Español**

```
Al iniciarse, la aplicación ya no anuncia el correo que llegó mientras estaba cerrada: las notificaciones son para el correo que llega con la aplicación abierta.
```

**Italiano**

```
All'avvio l'applicazione non annuncia più la posta arrivata mentre era chiusa: le notifiche riguardano la posta che arriva mentre è aperta.
```

**Português**

```
Ao arrancar, a aplicação deixa de anunciar o correio que chegou enquanto esteve fechada: as notificações são para o correio que chega com ela aberta.
```
