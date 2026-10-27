# A large Gmail mailbox finishes downloading, and carries on where it stopped

Platforms: all
Bump: patch

> Google bills a Gmail message fetch at 20 units against 6,000 per user per minute, so the
> engine's twenty-wide fan-out spent a minute's budget in seconds and every pass of a large
> mailbox was refused partway and started again from the top. The engine now paces Gmail under
> that budget, checkpoints a backfill after every page and resumes it from Gmail's page token,
> runs a delta before each page so new mail keeps arriving, and renews the sync lease for as
> long as a pass runs.

**English**

```
A Gmail account with a lot of mail now finishes downloading, continues where it stopped, and keeps receiving new mail meanwhile.
```

**Nederlands**

```
Een Gmail-account met veel e-mail wordt nu volledig gedownload, gaat verder waar het stopte en blijft intussen nieuwe e-mail ontvangen.
```

**Deutsch**

```
Ein Gmail-Konto mit vielen E-Mails wird jetzt vollständig heruntergeladen, setzt dort fort, wo es aufgehört hat, und empfängt währenddessen weiter neue E-Mails.
```

**Français**

```
Un compte Gmail contenant beaucoup d’e-mails se télécharge désormais entièrement, reprend là où il s’était arrêté et continue de recevoir les nouveaux messages.
```

**Español**

```
Una cuenta de Gmail con mucho correo ahora se descarga por completo, continúa donde se detuvo y sigue recibiendo correo nuevo mientras tanto.
```

**Italiano**

```
Un account Gmail con molta posta ora completa il download, riprende da dove si era fermato e continua a ricevere nuova posta nel frattempo.
```

**Português**

```
Uma conta Gmail com muito correio é agora transferida por completo, retoma onde parou e continua a receber correio novo entretanto.
```
