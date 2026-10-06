# The window stays responsive while the app opens its mailbox

Platforms: all
Bump: patch

> The core's constructor now runs off the UI thread on every client. On macOS and iOS it ran on
> the main actor, and on Linux before the window existed, so a store migration after an update
> froze the window (macOS reported the app as not responding). An open that outlasts 500 ms
> shows "Opening your mailbox…" with a progress indicator.

**English**

```
The app no longer freezes while it opens your mailbox after an update; it shows that it is opening it instead.
```

**Nederlands**

```
De app loopt niet meer vast terwijl hij na een update je mailbox opent; hij laat in plaats daarvan zien dat hij bezig is.
```

**Deutsch**

```
Die App friert nicht mehr ein, während sie nach einem Update Ihr Postfach öffnet; stattdessen zeigt sie an, dass sie es öffnet.
```

**Français**

```
L'app ne se fige plus pendant l'ouverture de votre boîte aux lettres après une mise à jour ; elle indique qu'elle l'ouvre.
```

**Español**

```
La app ya no se bloquea mientras abre tu buzón tras una actualización; en su lugar, indica que lo está abriendo.
```

**Italiano**

```
L'app non si blocca più mentre apre la tua casella di posta dopo un aggiornamento; mostra invece che la sta aprendo.
```

**Português**

```
A app já não fica bloqueada enquanto abre a sua caixa de correio após uma atualização; em vez disso, indica que a está a abrir.
```
