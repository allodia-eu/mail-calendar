# Gmail's inbox tabs, chat and extra stars are not folders

Platforms: all
Bump: patch

> Gmail lists its inbox tabs (`CATEGORY_*`), `CHAT` and its extra stars (`YELLOW_STAR`,
> `RED_BANG`, …) as labels, and the engine filed messages in them, so a Gmail account's folder
> tree drew each under its raw id. None is a place a user files mail in. The engine now leaves
> them out of the folder list and of every message's folders, the way it already treated read and
> starred. A message in a tab still carries the Inbox label; one archived from a tab is in All
> Mail. `NORMALIZER_VERSION` 12 has every account re-read its mail and calendars once on upgrade.

**English**

```
A Gmail account’s folder list no longer shows its inbox tabs, chat and extra stars as folders with names like CATEGORY_SOCIAL; that mail is still in the Inbox or All Mail.
```

**Nederlands**

```
De mappenlijst van een Gmail-account toont de inboxtabbladen, chats en extra sterren niet meer als mappen met namen als CATEGORY_SOCIAL; die e-mail staat nog steeds in Postvak IN of Alle e-mail.
```

**Deutsch**

```
Die Ordnerliste eines Gmail-Kontos zeigt die Posteingangs-Tabs, Chats und zusätzlichen Sterne nicht mehr als Ordner mit Namen wie CATEGORY_SOCIAL; diese E-Mails finden Sie weiterhin im Posteingang oder unter „Alle E-Mails“.
```

**Français**

```
La liste des dossiers d’un compte Gmail n’affiche plus les onglets de la boîte de réception, le chat et les étoiles supplémentaires comme des dossiers aux noms tels que CATEGORY_SOCIAL ; ces messages restent dans la Boîte de réception ou dans Tous les messages.
```

**Español**

```
La lista de carpetas de una cuenta de Gmail ya no muestra las pestañas de la bandeja de entrada, el chat ni las estrellas adicionales como carpetas con nombres como CATEGORY_SOCIAL; ese correo sigue en la Bandeja de entrada o en Todo el correo.
```

**Italiano**

```
L’elenco delle cartelle di un account Gmail non mostra più le schede della posta in arrivo, la chat e le stelle aggiuntive come cartelle con nomi come CATEGORY_SOCIAL; quei messaggi restano in Posta in arrivo o in Tutta la posta.
```

**Português**

```
A lista de pastas de uma conta Gmail já não mostra os separadores da caixa de entrada, o chat e as estrelas adicionais como pastas com nomes como CATEGORY_SOCIAL; esse correio continua na Caixa de entrada ou em Todo o correio.
```
