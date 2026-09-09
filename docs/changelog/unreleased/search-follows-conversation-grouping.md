# Search follows conversation grouping

Platforms: all
Bump: minor

> Search projected flat rows whatever the list was set to, so turning conversations on changed
> every list in the app except this one. It now goes through the same threaded projection the
> mailbox list uses, which needed no new ordering or grouping code: that projection already reads
> `AccountMessage::in_scope` as "belongs to the list being built", so a search marks its hits in
> scope and the rest of their conversations out of it. Three properties fall out of that one
> reading. A conversation is listed only if a message in it matched; it carries the whole thread,
> so a row means here what it means in the mailbox (`docs/list-selection.md`); and it is labelled
> by, and sorted on, its newest **matching** message, so an unrelated later reply neither carries
> an old thread to the top nor puts a subject on the row that answers nothing the user asked. The
> MCP surface stays flat on purpose: it answers with messages, paged by offset and addressed by
> key, and a conversation is not addressable that way.

**English**

```
Search results are now grouped into conversations when conversation grouping is on.
```

**Nederlands**

```
Zoekresultaten worden nu gegroepeerd in gesprekken wanneer groeperen op gesprek aanstaat.
```

**Deutsch**

```
Suchergebnisse werden jetzt zu Konversationen gruppiert, wenn die Konversationsgruppierung aktiv ist.
```

**Français**

```
Les résultats de recherche sont désormais regroupés en conversations lorsque le regroupement par conversation est activé.
```

**Español**

```
Los resultados de búsqueda ahora se agrupan en conversaciones cuando la agrupación por conversación está activada.
```

**Italiano**

```
I risultati di ricerca ora sono raggruppati in conversazioni quando il raggruppamento per conversazione è attivo.
```

**Português**

```
Os resultados da pesquisa passam a ser agrupados em conversas quando o agrupamento por conversa está ativo.
```
