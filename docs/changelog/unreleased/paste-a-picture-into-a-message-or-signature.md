# Paste a picture into a message, and into a signature

Platforms: linux
Bump: patch

> WebKitGTK's paste `DataTransfer` answers `getData` for the text flavours and nothing else:
> `types`, `items` and `files` come back empty whatever is on the clipboard, and
> `getData("image/png")` is the empty string. So the shared bundle's paste handler, which every
> other host feeds a picture through, saw one here and inserted nothing, while a drop of the same
> file worked. The page cannot rescue it either: letting WebKit's own paste run yields
> `<img src="blob:…">`, and the composer's `connect-src 'none'` blocks `fetch` and
> `XMLHttpRequest` on that blob, leaving only a canvas re-encode that throws the original bytes
> away. The host now reads the clipboard itself and feeds the picture to the same
> `insertComposerImage` seam a dropped one arrives by, with the bytes sniffed against the core's
> own closed raster set and size cap, reached through a byte-shaped twin of the call a dropped file
> makes. Both routes into a paste are covered, the chord and the context menu's own Paste, and a
> paste carrying no picture is left to the page untouched, so pasting text is unchanged and so is
> the plain-text paste Ctrl+Shift+V asks for. Two traps cost a round of testing each and are now
> asserted: the chord controller has to sit on an ancestor, because WebKitGTK's own key controller
> is on the view and one added there is behind it; and a file manager's Copy puts no `image/*` on
> the clipboard at all, only `text/uri-list` and the portal's transfer types, so a copied picture
> arrives as the same `GdkFileList` a drop carries. A pasted picture is shown in the message
> without being asked about, unlike a dropped one: a paste is aimed at the caret and has already
> said what it is for. The Settings signature editor is the same bundle against the same toolkit, so
> it takes a pasted picture too; the clipboard is read once, in one place, and each editor is handed
> the bytes, because a signature's size rule is its own and far tighter than a message body's.
> The trail all of this leaves is `debug`: a paste that works is not a support event, and the
> refusals that matter are already logged by the core.

**English**

```
Pasting a picture into a message or a signature now puts it in place, instead of doing nothing, whether you copied the picture itself or the file it is in.
```

**Nederlands**

```
Een afbeelding plakken in een bericht of handtekening zet deze nu op zijn plek in plaats van niets te doen, of u nu de afbeelding zelf of het bestand ervan kopieerde.
```

**Deutsch**

```
Ein eingefügtes Bild landet jetzt in der Nachricht oder der Signatur, statt wirkungslos zu bleiben, gleich ob Sie das Bild selbst oder dessen Datei kopiert haben.
```

**Français**

```
Coller une image dans un message ou une signature l'insère désormais au bon endroit au lieu de ne rien faire, que vous ayez copié l'image elle-même ou son fichier.
```

**Español**

```
Al pegar una imagen en un mensaje o una firma ahora se coloca en su sitio en vez de no hacer nada, tanto si copiaste la imagen como su archivo.
```

**Italiano**

```
Incollare un'immagine in un messaggio o in una firma ora la inserisce al suo posto invece di non fare nulla, sia che tu abbia copiato l'immagine o il suo file.
```

**Português**

```
Colar uma imagem numa mensagem ou numa assinatura passa a colocá-la no sítio em vez de não fazer nada, quer tenha copiado a imagem quer o respetivo ficheiro.
```
