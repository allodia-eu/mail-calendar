# The unread count on the Dock icon again

Platforms: macos
Bump: patch

> On current macOS the Dock badge is the notification centre's badge count, drawn under the
> `.badge` permission the app already asks for; a write to the dock tile's own label alone is
> dropped when the app holds no badge setting. `AppBadge` now writes both, so the OS draws the
> one it honours. The badge still answers to System Settings → Notifications → the app's "Badge
> application icon" alone: with the alert locations off the icon shows the count and no banner
> appears (docs/app-badge.md, rule 6). Verified by eye on a current macOS dev build with unread
> mail in the harness account: the Dock icon shows the count, and clears at zero.

**English**

```
Fixed: the unread count shows on the app's Dock icon again.
```

**Nederlands**

```
Opgelost: het aantal ongelezen berichten verschijnt opnieuw op het Dock-pictogram van de app.
```

**Deutsch**

```
Behoben: Die Anzahl ungelesener Nachrichten wird wieder auf dem Dock-Symbol der App angezeigt.
```

**Français**

```
Corrigé : le nombre de messages non lus s'affiche à nouveau sur l'icône Dock de l'app.
```

**Español**

```
Corregido: el número de mensajes no leídos vuelve a aparecer en el icono de la app en el Dock.
```

**Italiano**

```
Corretto: il numero di messaggi non letti torna a essere mostrato sull'icona dell'app nel Dock.
```

**Português**

```
Corrigido: o número de mensagens por ler volta a aparecer no ícone da app na Dock.
```
