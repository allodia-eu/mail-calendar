# Archiving applies instead of returning the message to the list

Platforms: all
Bump: patch

> The inline outbox drivers resolve the op they just enqueued, but reached it through the engine's
> *batch* claim: ordered by id, capped at sixteen, leasing whatever it got. The driver's own op is
> always last. So a batch leased up to fifteen ops nobody would resolve, holding each one's message
> for the lease's whole term, and no batch reached an op past the cap: once an account carried more
> than sixteen unresolved ops, every later write was refused and each refusal left one more behind.
> Separately, a message another write already held surfaced as a hard failure rather than a wait,
> and marking a message read when it opens puts exactly that write in flight, so archiving a moment
> later collided with it. Each collision left one op stuck, and after enough of them the account
> stopped accepting writes at all. Fixed in the engine: a driver now claims its own op by id, and
> waits for a message another write holds instead of giving up. An account already in that state
> recovers on its own, because a lease that has expired holds nothing.

**English**

```
Archiving, deleting or flagging a message now applies reliably, instead of the row leaving the list and returning a moment later.
```

**Nederlands**

```
Archiveren, verwijderen of markeren van een bericht werkt nu betrouwbaar, in plaats van dat de regel uit de lijst verdwijnt en even later terugkomt.
```

**Deutsch**

```
Das Archivieren, Löschen oder Markieren einer Nachricht wird jetzt zuverlässig übernommen, statt dass die Zeile aus der Liste verschwindet und kurz darauf zurückkehrt.
```

**Français**

```
Archiver, supprimer ou marquer un message s'applique désormais de façon fiable, au lieu de voir la ligne quitter la liste puis y revenir un instant plus tard.
```

**Español**

```
Archivar, eliminar o marcar un mensaje ahora se aplica de forma fiable, en vez de que la fila desaparezca de la lista y vuelva un momento después.
```

**Italiano**

```
Archiviare, eliminare o contrassegnare un messaggio ora viene applicato in modo affidabile, invece di far sparire la riga dall'elenco per poi vederla ricomparire poco dopo.
```

**Português**

```
Arquivar, eliminar ou marcar uma mensagem passa a aplicar-se de forma fiável, em vez de a linha sair da lista e voltar pouco depois.
```
