# Linux frees a closed composer's memory, and a new composer always draws

Platforms: linux
Bump: patch

> Every composer opened, and every message printed, kept a process of its own running after it
> closed, about 180 MB each. Once enough had piled up, a new composer could open with an empty
> space where the message belongs.

**English**

```
Closing a message you were writing, or printing one, now frees its memory, and a new message no longer opens without its writing area after many have been opened.
```

**Nederlands**

```
Een bericht sluiten dat je aan het schrijven was, of er een afdrukken, geeft het geheugen nu weer vrij, en een nieuw bericht opent niet meer zonder schrijfvak nadat er veel geopend zijn.
```

**Deutsch**

```
Das Schließen einer Nachricht, die Sie geschrieben haben, oder das Drucken einer Nachricht gibt den Speicher jetzt wieder frei, und eine neue Nachricht öffnet sich nach vielen geöffneten nicht mehr ohne Schreibbereich.
```

**Français**

```
Fermer un message en cours de rédaction, ou en imprimer un, libère désormais sa mémoire, et un nouveau message ne s’ouvre plus sans sa zone de saisie après en avoir ouvert beaucoup.
```

**Español**

```
Cerrar un mensaje que estabas escribiendo, o imprimir uno, ahora libera su memoria, y un mensaje nuevo ya no se abre sin su área de escritura después de abrir muchos.
```

**Italiano**

```
Chiudere un messaggio che stavi scrivendo, o stamparne uno, ora libera la sua memoria, e un nuovo messaggio non si apre più senza l’area di scrittura dopo averne aperti molti.
```

**Português**

```
Fechar uma mensagem que estava a escrever, ou imprimir uma, liberta agora a sua memória, e uma mensagem nova já não abre sem a área de escrita depois de se abrirem muitas.
```
