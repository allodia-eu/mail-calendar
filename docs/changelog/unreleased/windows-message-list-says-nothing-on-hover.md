# The Windows message list says nothing under the pointer

Platforms: windows
Bump: patch

> The list carries keyboard accelerators for Delete, Back and Escape (`docs/list-selection.md`,
> rule 9), and `KeyboardAcceleratorPlacementMode` defaults to `Auto`, which tells the framework to
> raise a tooltip naming the accelerator's **key** over whatever the pointer is resting on. So a
> mouse crossing the mailbox trailed a bare "Delete" down it, over the rows being scanned. Nothing
> in this repo could see it: the string is the framework's word for the key rather than anything in
> the catalog, it is in no markup, no binding assigns it, and it matches no action on the row's own
> menu, which calls that one "Move to Trash". `Hidden` decides whether an accelerator is announced,
> never whether it fires, and the new suite holds both halves so a later change cannot buy the
> silence by dropping the keys.

**English**

```
Moving the mouse down the message list no longer trails a "Delete" label across the messages being scanned.
```

**Nederlands**

```
De muis over de berichtenlijst bewegen laat niet langer het label "Delete" over de berichten meelopen.
```

**Deutsch**

```
Die Maus über die Nachrichtenliste zu bewegen zieht nicht mehr die Beschriftung "Delete" über die Nachrichten mit.
```

**Français**

```
Déplacer la souris dans la liste des messages ne fait plus glisser l'étiquette « Delete » par-dessus les messages parcourus.
```

**Español**

```
Mover el ratón por la lista de mensajes ya no arrastra la etiqueta «Delete» por encima de los mensajes.
```

**Italiano**

```
Muovere il mouse sull'elenco dei messaggi non trascina più l'etichetta "Delete" sopra i messaggi.
```

**Português**

```
Mover o rato pela lista de mensagens deixa de arrastar a etiqueta "Delete" por cima das mensagens.
```
