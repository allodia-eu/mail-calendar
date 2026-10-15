# Pasting lists from Word, merged cells and pictures

Platforms: all
Bump: patch

> Closes the four paste gaps composer-security.md listed. Word's list paragraphs (`mso-list`)
> become real lists, keeping where an interrupted list's numbering carries on (`List.start`);
> a table cell merged across rows no longer shifts the row below it; and a
> picture the pasted markup carries inline (`data:`) is kept, so text and a picture copied together
> inside a message move together. A picture the markup names by URL is still dropped, now stated
> as a rule, as is the plain text an Android keyboard's clipboard strip types in. The empty
> paragraph Chromium's insertHTML leaves after a pasted list or table is no longer sent as a blank
> line. On iPhone and iPad the composer reads the clipboard's HTML itself, because iOS WebKit
> rewrites another app's HTML and drops `mso-list`; there and on Linux the core decodes it.

**English**

```
Lists pasted from Word or Outlook stay lists with no blank line after them, merged table cells no longer shift the rows below, and a picture copied with text inside a message moves with it.
```

**Nederlands**

```
Lijsten die je uit Word of Outlook plakt blijven lijsten zonder lege regel erna, samengevoegde tabelcellen verschuiven de rijen eronder niet meer, en een afbeelding die je met tekst in een bericht kopieert, gaat mee.
```

**Deutsch**

```
Aus Word oder Outlook eingefügte Listen bleiben Listen ohne Leerzeile danach, verbundene Tabellenzellen verschieben die Zeilen darunter nicht mehr, und ein mit Text kopiertes Bild wird mitgenommen.
```

**Français**

```
Les listes collées depuis Word ou Outlook restent des listes, sans ligne vide après, les cellules fusionnées ne décalent plus les lignes en dessous, et une image copiée avec du texte suit le texte.
```

**Español**

```
Las listas que pegas desde Word u Outlook siguen siendo listas, sin línea en blanco después, las celdas combinadas ya no desplazan las filas de debajo, y una imagen copiada con texto se mueve con él.
```

**Italiano**

```
Gli elenchi incollati da Word o Outlook restano elenchi, senza una riga vuota dopo, le celle unite non spostano più le righe sottostanti, e un'immagine copiata con il testo si sposta con esso.
```

**Português**

```
As listas coladas do Word ou do Outlook continuam a ser listas, sem linha em branco depois, as células unidas já não deslocam as linhas abaixo, e uma imagem copiada com texto acompanha-o.
```
