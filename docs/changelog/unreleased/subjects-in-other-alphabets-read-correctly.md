# Subjects in other alphabets read correctly

Platforms: all
Bump: patch

> A Japanese subject and sender name arrived as `$B...(B` in the notification and the list
> row. Header text is RFC 2047 encoded, and the engine's IMAP adapter read every charset it
> did not know as UTF-8: `ISO-2022-JP` is 7-bit, so that read produces no replacement
> character to notice, just the escape sequences as text. The surveyed sibling was worse:
> nothing decoded Gmail's headers at all, since its API returns them raw, so a non-ASCII
> Gmail subject showed the literal `=?iso-2022-jp?b?...?=`. Both now go through one decoder
> in `engine-mime` over `mail-parser`'s charset table. JMAP and Graph were never affected;
> their servers decode. The decode happens as mail is synced, so text already stored stays
> wrong until that folder is synced again, which is what the note says.

**English**

```
Subjects and sender names in alphabets such as Japanese now read correctly; mail synced earlier corrects itself on the next full sync.
```

**Nederlands**

```
Onderwerpen en afzendernamen in bijvoorbeeld het Japans worden nu correct weergegeven; eerder opgehaalde berichten herstellen zich bij de volgende volledige synchronisatie.
```

**Deutsch**

```
Betreffzeilen und Absendernamen etwa in japanischer Schrift werden jetzt korrekt angezeigt; zuvor abgerufene Nachrichten korrigieren sich bei der nächsten vollständigen Synchronisierung.
```

**Français**

```
Les objets et les noms d'expéditeurs écrits par exemple en japonais s'affichent désormais correctement ; les messages déjà récupérés se corrigent à la prochaine synchronisation complète.
```

**Español**

```
Los asuntos y los nombres de remitente escritos, por ejemplo, en japonés ya se muestran correctamente; los mensajes ya descargados se corrigen en la próxima sincronización completa.
```

**Italiano**

```
Gli oggetti e i nomi dei mittenti scritti ad esempio in giapponese ora vengono visualizzati correttamente; i messaggi già scaricati si correggono alla successiva sincronizzazione completa.
```

**Português**

```
Os assuntos e os nomes dos remetentes escritos, por exemplo, em japonês passam a ser apresentados corretamente; as mensagens já descarregadas corrigem-se na próxima sincronização completa.
```
