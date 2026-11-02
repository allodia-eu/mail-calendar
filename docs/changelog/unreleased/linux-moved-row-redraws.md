# A row that moves and changes is redrawn on Linux

Platforms: linux
Bump: patch

> The message list reconciles in place, and a row whose key moved position had its widget
> reparented without comparing its rendering, so it kept what it showed before. Leaving the Inbox
> for Drafts did this to a conversation holding a draft: it moved to the top of Drafts still
> showing the received message's subject and sender rather than the draft's. A moved row is now
> rebuilt when its rendering changed, as one that stayed in place already was.

**English**

```
A conversation no longer keeps the subject and sender another folder lists it by when you switch folders; in Drafts it shows its draft.
```

**Nederlands**

```
Een gesprek houdt bij het wisselen van map niet langer het onderwerp en de afzender van een andere map; in Concepten toont het zijn concept.
```

**Deutsch**

```
Eine Unterhaltung behält beim Wechsel des Ordners nicht mehr Betreff und Absender eines anderen Ordners; in Entwürfe zeigt sie ihren Entwurf.
```

**Français**

```
Une conversation ne garde plus l’objet et l’expéditeur d’un autre dossier lorsque vous changez de dossier ; dans Brouillons, elle affiche son brouillon.
```

**Español**

```
Una conversación ya no conserva el asunto y el remitente de otra carpeta al cambiar de carpeta; en Borradores muestra su borrador.
```

**Italiano**

```
Una conversazione non mantiene più l’oggetto e il mittente di un’altra cartella quando cambi cartella; in Bozze mostra la sua bozza.
```

**Português**

```
Uma conversa já não mantém o assunto e o remetente de outra pasta quando muda de pasta; em Rascunhos mostra o seu rascunho.
```
