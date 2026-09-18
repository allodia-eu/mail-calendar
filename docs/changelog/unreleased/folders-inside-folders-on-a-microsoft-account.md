# Folders inside folders on a Microsoft account

Platforms: all
Bump: patch

> Graph lists a mailbox one level at a time: `GET /me/mailFolders` answers with the folders the
> mailbox root holds and nothing below them. The engine asked once and stopped there, so a folder
> filed under another folder was never listed, and because a folder's mail is only fetched for a
> folder the list named, none of that mail was reachable either. On the mailbox this was reported
> from, 29 of 53 folders were missing, 26 of them under one folder. Fixed in the engine
> (allodia-eu/email-calendar-sync-engine#213), which now walks the tree, so the pane fills on the
> next folder sync with no reset.

**English**

```
A Microsoft account now shows the folders filed inside other folders, and the mail in them, instead of only the folders at the top of the mailbox.
```

**Nederlands**

```
Een Microsoft-account laat nu ook de mappen in andere mappen zien, en de e-mail erin, in plaats van alleen de mappen bovenaan het postvak.
```

**Deutsch**

```
Ein Microsoft-Konto zeigt jetzt auch die Ordner in anderen Ordnern und die E-Mails darin, statt nur die Ordner auf oberster Ebene.
```

**Français**

```
Un compte Microsoft affiche désormais les dossiers rangés dans d'autres dossiers, et les messages qu'ils contiennent, au lieu des seuls dossiers du premier niveau.
```

**Español**

```
Una cuenta de Microsoft ahora muestra las carpetas guardadas dentro de otras carpetas, y el correo que contienen, en vez de solo las carpetas del primer nivel.
```

**Italiano**

```
Un account Microsoft ora mostra anche le cartelle contenute in altre cartelle, e la posta al loro interno, invece delle sole cartelle di primo livello.
```

**Português**

```
Uma conta Microsoft mostra agora as pastas guardadas dentro de outras pastas, e o correio que têm, em vez de apenas as pastas do primeiro nível.
```
