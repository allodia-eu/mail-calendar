# Sync every folder, not only the ones the server gives a role

Platforms: all
Bump: patch

> An IMAP or Microsoft account bound a provider only to the Inbox and the folders the server tagged
> Sent, Drafts, Trash, Archive or Junk, so every other folder synced once, when it was opened, and
> no pass named it again. A server that tags its Archive `\All` (a real one does) lost that folder
> too, which is where archiving files mail. The dial now binds every folder the account lists
> except `\Flagged` and `\Important`, which are views of mail filed elsewhere; a pass binds and
> syncs a folder listed since the account connected; and an account on push also runs a full pass
> every 15 minutes, because a watch covers only the folders it names.

**English**

```
Every folder of an IMAP or Microsoft account now syncs in the background within the sync depth you chose, not only the Inbox and the standard folders; on push, the folders not watched are checked every 15 minutes.
```

**Nederlands**

```
Alle mappen van een IMAP- of Microsoft-account worden nu op de achtergrond gesynchroniseerd binnen de synchronisatiediepte die je hebt gekozen, niet alleen Postvak IN en de standaardmappen; bij push worden de mappen die niet worden gevolgd elke 15 minuten gecontroleerd.
```

**Deutsch**

```
Alle Ordner eines IMAP- oder Microsoft-Kontos werden jetzt im Hintergrund innerhalb der von Ihnen gewählten Synchronisationstiefe synchronisiert, nicht nur der Posteingang und die Standardordner; bei Push werden die nicht überwachten Ordner alle 15 Minuten abgerufen.
```

**Français**

```
Tous les dossiers d'un compte IMAP ou Microsoft se synchronisent désormais en arrière-plan dans la profondeur de synchronisation que vous avez choisie, et plus seulement la boîte de réception et les dossiers standard ; en push, les dossiers non suivis sont vérifiés toutes les 15 minutes.
```

**Español**

```
Todas las carpetas de una cuenta IMAP o Microsoft se sincronizan ahora en segundo plano dentro de la profundidad de sincronización elegida, no solo la bandeja de entrada y las carpetas estándar; con push, las carpetas no vigiladas se comprueban cada 15 minutos.
```

**Italiano**

```
Tutte le cartelle di un account IMAP o Microsoft ora si sincronizzano in background entro la profondità di sincronizzazione scelta, non solo la posta in arrivo e le cartelle standard; con il push, le cartelle non monitorate vengono controllate ogni 15 minuti.
```

**Português**

```
Todas as pastas de uma conta IMAP ou Microsoft são agora sincronizadas em segundo plano dentro da profundidade de sincronização escolhida, e não apenas a caixa de entrada e as pastas padrão; com push, as pastas não vigiadas são verificadas a cada 15 minutos.
```
