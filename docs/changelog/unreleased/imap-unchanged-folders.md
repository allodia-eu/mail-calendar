# Unchanged Yahoo folders sync faster; IMAP accounts re-sync once

Platforms: all
Bump: patch

> Two engine changes. On Yahoo, a folder whose change counter (`HIGHESTMODSEQ`) and next UID
> are unchanged is no longer re-read message by message: 0.4 s instead of about 6 s for a
> 2584-message inbox. And the IMAP cursor now records the sync's version, so every IMAP folder
> synced by an earlier build is synced afresh once, to its sync depth: that fetches the mail a
> Yahoo window cut at 1000 never downloaded and corrects stale read, flag and delete state.

**English**

```
Yahoo folders without changes now sync faster, and after updating, IMAP accounts download their message list once more.
```

**Nederlands**

```
Yahoo-mappen zonder wijzigingen synchroniseren nu sneller, en na de update downloaden IMAP-accounts hun berichtenlijst eenmalig opnieuw.
```

**Deutsch**

```
Yahoo-Ordner ohne Änderungen werden jetzt schneller synchronisiert, und nach dem Update laden IMAP-Konten ihre Nachrichtenliste einmalig neu.
```

**Français**

```
Les dossiers Yahoo sans modification se synchronisent désormais plus vite, et après la mise à jour, les comptes IMAP retéléchargent une fois leur liste de messages.
```

**Español**

```
Las carpetas de Yahoo sin cambios se sincronizan ahora más rápido y, tras la actualización, las cuentas IMAP vuelven a descargar una vez su lista de mensajes.
```

**Italiano**

```
Le cartelle Yahoo senza modifiche ora si sincronizzano più rapidamente e, dopo l'aggiornamento, gli account IMAP scaricano di nuovo una volta l'elenco dei messaggi.
```

**Português**

```
As pastas do Yahoo sem alterações passam a sincronizar mais depressa e, após a atualização, as contas IMAP voltam a transferir uma vez a lista de mensagens.
```
