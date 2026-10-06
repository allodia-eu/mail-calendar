# Read and flag changes show at once; a write the server cannot take yet is kept

Platforms: all
Bump: patch

> The engine now shows a queued keyword change in the store from the moment it is queued and
> takes it back only when the server refuses it for good, and its send step tells a refusal for
> now (offline, rate limited) from one for good. The app redraws as soon as a read or flag change
> is queued, instead of after the follow-up sync (about 9 s on a Yahoo inbox), and no longer puts
> back a row or a flag whose write is only waiting: before, a rate-limited archive reappeared and
> was then carried out minutes later.

**English**

```
Marking a message as read or flagged now shows straight away, and an action the server cannot take right now, offline or rate limited, is kept and sent later instead of being undone.
```

**Nederlands**

```
Een bericht als gelezen markeren of een markering geven is nu meteen zichtbaar, en een actie die de server op dat moment niet kan verwerken, offline of door een limiet, blijft staan en wordt later verstuurd in plaats van teruggedraaid.
```

**Deutsch**

```
Eine Nachricht als gelesen oder markiert zu kennzeichnen, ist jetzt sofort sichtbar, und eine Aktion, die der Server gerade nicht annehmen kann, offline oder wegen einer Begrenzung, bleibt erhalten und wird später gesendet, statt rückgängig gemacht zu werden.
```

**Français**

```
Marquer un message comme lu ou le signaler s'affiche désormais immédiatement, et une action que le serveur ne peut pas traiter pour l'instant, hors ligne ou limitée, est conservée et envoyée plus tard au lieu d'être annulée.
```

**Español**

```
Marcar un mensaje como leído o marcarlo ahora se ve al instante, y una acción que el servidor no puede aceptar en ese momento, sin conexión o por un límite, se conserva y se envía más tarde en lugar de deshacerse.
```

**Italiano**

```
Segnare un messaggio come letto o contrassegnarlo ora si vede subito, e un'azione che il server non può accettare in quel momento, offline o per un limite, viene conservata e inviata più tardi invece di essere annullata.
```

**Português**

```
Marcar uma mensagem como lida ou sinalizá-la passa a ver-se de imediato, e uma ação que o servidor não pode aceitar nesse momento, sem ligação ou por um limite, é mantida e enviada mais tarde em vez de ser anulada.
```
