# Google contacts keep updating after a week without a sync

Platforms: all
Bump: patch

> Google's People API lets a contacts sync token lapse seven days after the sync that issued it,
> and answers the next request with `400 FAILED_PRECONDITION` and the reason `EXPIRED_SYNC_TOKEN`
> rather than the `410` its Calendar API sends. The engine read that as a conflict, so it sent the
> lapsed token again on every pass and the address book stopped updating for good. It now starts
> that address book over with a full read (allodia-eu/email-calendar-sync-engine#304). An address
> book already stuck recovers at the first sync after the update.

**English**

```
Google contacts no longer stop updating when the app has gone a week or more without syncing them.
```

**Nederlands**

```
Google-contacten blijven nu bijgewerkt, ook als de app ze een week of langer niet heeft gesynchroniseerd.
```

**Deutsch**

```
Google-Kontakte werden jetzt weiter aktualisiert, auch wenn die App sie eine Woche oder länger nicht synchronisiert hat.
```

**Français**

```
Les contacts Google continuent désormais d’être mis à jour, même si l’app ne les a pas synchronisés depuis une semaine ou plus.
```

**Español**

```
Los contactos de Google ya siguen actualizándose aunque la aplicación lleve una semana o más sin sincronizarlos.
```

**Italiano**

```
I contatti Google ora continuano ad aggiornarsi anche se l’app non li sincronizza da una settimana o più.
```

**Português**

```
Os contactos Google continuam agora a ser atualizados, mesmo que a app não os sincronize há uma semana ou mais.
```
