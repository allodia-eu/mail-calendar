# A pushed message waits for a busy folder

Platforms: all
Bump: patch

> An IMAP watch's refresh gave up when another sync held the folder, and that sync may have read
> the server before the message landed, so the message stayed unseen until the next change or
> poll. The refresh now waits for the folder, on a task of its own so the watch stays in IDLE
> meanwhile, and changes that arrive while it waits are read by one further refresh.

**English**

```
A message that arrived while another sync was running no longer stays hidden until the next check.
```

**Nederlands**

```
Een bericht dat binnenkwam terwijl een andere synchronisatie liep, blijft niet langer verborgen tot de volgende controle.
```

**Deutsch**

```
Eine Nachricht, die während einer laufenden Synchronisierung eintraf, bleibt nicht mehr bis zur nächsten Prüfung verborgen.
```

**Français**

```
Un message arrivé pendant une autre synchronisation ne reste plus masqué jusqu’à la vérification suivante.
```

**Español**

```
Un mensaje que llegó mientras se ejecutaba otra sincronización ya no queda oculto hasta la siguiente comprobación.
```

**Italiano**

```
Un messaggio arrivato durante un’altra sincronizzazione non resta più nascosto fino al controllo successivo.
```

**Português**

```
Uma mensagem que chegou enquanto outra sincronização decorria já não fica oculta até à verificação seguinte.
```
