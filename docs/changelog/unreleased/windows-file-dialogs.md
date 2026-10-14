# File dialogs on Windows

Platforms: windows
Bump: patch

> The file pickers move to the Windows App SDK's own (`Microsoft.Windows.Storage.Pickers`), each
> owned by the window it was opened from: a Save opened from a message in its own window used to
> open over the mailbox. A save that failed before writing anything used to delete the file the
> user had chosen to replace.

**English**

```
Save and open dialogs appear over the window you opened them from, and a save that fails no longer removes the file you chose to replace.
```

**Nederlands**

```
Dialoogvensters voor opslaan en openen verschijnen boven het venster waaruit je ze opende, en een mislukte opslag verwijdert het bestand dat je wilde vervangen niet meer.
```

**Deutsch**

```
Dialoge zum Speichern und Öffnen erscheinen über dem Fenster, aus dem Sie sie geöffnet haben, und ein fehlgeschlagenes Speichern entfernt die Datei, die Sie ersetzen wollten, nicht mehr.
```

**Français**

```
Les fenêtres d'enregistrement et d'ouverture s'affichent au-dessus de la fenêtre d'où vous les avez ouvertes, et un enregistrement qui échoue ne supprime plus le fichier que vous vouliez remplacer.
```

**Español**

```
Los diálogos de guardar y abrir aparecen sobre la ventana desde la que los abriste, y un guardado que falla ya no elimina el archivo que querías reemplazar.
```

**Italiano**

```
Le finestre di salvataggio e apertura compaiono sopra la finestra da cui le hai aperte, e un salvataggio non riuscito non elimina più il file che volevi sostituire.
```

**Português**

```
As janelas de guardar e abrir aparecem sobre a janela a partir da qual as abriu, e uma gravação que falha já não remove o ficheiro que escolheu substituir.
```
