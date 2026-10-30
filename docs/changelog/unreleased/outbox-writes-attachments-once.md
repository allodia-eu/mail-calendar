# Save drafts with attachments without writing them again at every step

Platforms: all
Bump: patch

> The engine's outbox kept each op's payload in its row, so every claim and outcome rewrote it
> (a draft's payload carries its attachments), and kept it after the op settled: one store held
> 707 MB of settled payloads in 1.5 GB. The payload now lives beside the row, written once, and
> goes when the op settles (engine schema v20). The freed space is reused before the store grows
> and given back to the disk at the next compaction.

**English**

```
Saving a draft with attachments writes much less to disk, and a draft or message is no longer kept on the device a second time once the server has it.
```

**Nederlands**

```
Een concept met bijlagen opslaan schrijft veel minder naar de schijf, en een concept of bericht wordt niet meer een tweede keer op het apparaat bewaard zodra de server het heeft.
```

**Deutsch**

```
Einen Entwurf mit Anhängen zu speichern schreibt viel weniger auf den Datenträger, und ein Entwurf oder eine Nachricht wird nicht mehr ein zweites Mal auf dem Gerät aufbewahrt, sobald der Server sie hat.
```

**Français**

```
Enregistrer un brouillon avec des pièces jointes écrit beaucoup moins sur le disque, et un brouillon ou un message n'est plus conservé une seconde fois sur l'appareil une fois que le serveur l'a reçu.
```

**Español**

```
Guardar un borrador con adjuntos escribe mucho menos en el disco, y un borrador o mensaje ya no se guarda una segunda vez en el dispositivo cuando el servidor ya lo tiene.
```

**Italiano**

```
Salvare una bozza con allegati scrive molto meno sul disco, e una bozza o un messaggio non viene più conservato una seconda volta sul dispositivo quando il server lo ha già.
```

**Português**

```
Guardar um rascunho com anexos escreve muito menos no disco, e um rascunho ou mensagem deixa de ser guardado uma segunda vez no dispositivo assim que o servidor o tem.
```
