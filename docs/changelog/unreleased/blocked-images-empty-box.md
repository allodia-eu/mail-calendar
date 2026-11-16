# A blocked image leaves an empty space

Platforms: all
Bump: patch

> The reading document now comes from the engine's `engine-html`, which hides every image that is
> not inline while remote images are blocked (`visibility:hidden`), so its box keeps the layout and
> the platform's broken-image icon and alt text no longer show. An unresolved `cid:` picture is
> hidden the same way. Loading images for the message brings them back as before.

**English**

```
While remote images are blocked, a message now leaves an empty space where each one goes, rather than a broken-image icon.
```

**Nederlands**

```
Zolang externe afbeeldingen geblokkeerd zijn, laat een bericht nu een lege ruimte op de plek van elke afbeelding, in plaats van een pictogram voor een kapotte afbeelding.
```

**Deutsch**

```
Solange externe Bilder blockiert sind, zeigt eine Nachricht an ihrer Stelle jetzt einen leeren Bereich statt eines Symbols für ein defektes Bild.
```

**Français**

```
Tant que les images distantes sont bloquées, un message laisse désormais un espace vide à la place de chacune, au lieu d’une icône d’image cassée.
```

**Español**

```
Mientras las imágenes remotas estén bloqueadas, un mensaje deja ahora un espacio vacío en el lugar de cada una, en vez de un icono de imagen rota.
```

**Italiano**

```
Finché le immagini remote sono bloccate, un messaggio ora lascia uno spazio vuoto al posto di ciascuna, invece dell’icona di un’immagine danneggiata.
```

**Português**

```
Enquanto as imagens remotas estiverem bloqueadas, uma mensagem deixa agora um espaço vazio no lugar de cada uma, em vez de um ícone de imagem danificada.
```
