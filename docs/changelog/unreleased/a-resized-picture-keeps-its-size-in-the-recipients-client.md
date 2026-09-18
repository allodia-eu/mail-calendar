# A resized picture keeps its size in the recipient's client

Platforms: all
Bump: patch

> The size rode out as the `width` **attribute** alone. That attribute is a presentational hint,
> which the cascade places below every author stylesheet rule, so a recipient whose client carries
> so much as an `img { width: … }` rule of its own silently overrode it and the reader saw a size
> the sender never chose. It is now also an inline `style`, which is an author declaration and
> loses only to `!important`; the attribute stays for the older Word-based Outlook, which reads it
> and not the style. That is what Outlook itself sends, and it is what this renderer already does
> everywhere else, since mail clients strip `<head>`/`<style>`.
>
> `max-width: 100%` goes on every inline picture, whether or not a width was chosen, and the
> picture nobody resized is the case it matters for: with no width to state it rendered at its
> intrinsic size, and a photograph off a phone is thousands of pixels wide. Our own reading view
> caps every image, so a message that overflowed a narrower client looked correct to us. No
> `height` is emitted: the document carries a width alone, and the height follows the picture's own
> ratio rather than being guessed from bytes the renderer never decodes.

**English**

```
A picture you resized now arrives at that size in the recipient's mail app, and one you did not resize no longer runs off the side of a narrow window.
```

**Nederlands**

```
Een afbeelding waarvan u het formaat wijzigde komt nu op dat formaat aan in de mailapp van de ontvanger, en een afbeelding die u niet aanpaste valt niet meer buiten de rand van een smal venster.
```

**Deutsch**

```
Ein Bild, dessen Größe Sie geändert haben, kommt jetzt in dieser Größe in der Mail-App der Empfängerin an, und ein Bild ohne Größenänderung läuft nicht mehr über den Rand eines schmalen Fensters hinaus.
```

**Français**

```
Une image dont vous avez changé la taille arrive désormais à cette taille dans l'application de messagerie du destinataire, et une image que vous n'avez pas redimensionnée ne dépasse plus du bord d'une fenêtre étroite.
```

**Español**

```
Una imagen a la que cambiaste el tamaño ahora llega con ese tamaño a la aplicación de correo de quien la recibe, y una que no redimensionaste ya no se sale del borde de una ventana estrecha.
```

**Italiano**

```
Un'immagine di cui hai cambiato le dimensioni ora arriva con quelle dimensioni nell'app di posta di chi la riceve, e una che non hai ridimensionato non esce più dal bordo di una finestra stretta.
```

**Português**

```
Uma imagem cujo tamanho alterou chega agora com esse tamanho à aplicação de correio de quem a recebe, e uma que não redimensionou já não passa da margem de uma janela estreita.
```
