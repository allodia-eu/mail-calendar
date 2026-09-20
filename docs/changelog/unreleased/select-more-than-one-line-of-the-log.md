# Select more than one line of the diagnostic log

Platforms: macos
Bump: patch

> The viewer drew one `Text` per line inside a lazy list, and SwiftUI scopes text selection to a
> single `Text`, so a drag could never cross lines: a reader copying a failure into a support
> request got the one line and none of the context around it. macOS now lays the log out as one
> `NSTextView` document, which selects across the whole file and costs nothing at this size, since
> TextKit lays out what is on screen rather than what is loaded. Read-only, monospace, still opening
> at the end with the jump-to-end button. iOS and Android keep the per-row list and the same limit,
> recorded under the contract's known gaps.

**English**

```
You can now select and copy more than one line at a time in Settings → Diagnostics.
```

**Nederlands**

```
U kunt nu meerdere regels tegelijk selecteren en kopiëren in Instellingen → Diagnostiek.
```

**Deutsch**

```
Sie können jetzt mehrere Zeilen auf einmal auswählen und kopieren unter Einstellungen → Diagnose.
```

**Français**

```
Vous pouvez maintenant sélectionner et copier plusieurs lignes à la fois dans Réglages → Diagnostic.
```

**Español**

```
Ahora puedes seleccionar y copiar más de una línea a la vez en Ajustes → Diagnóstico.
```

**Italiano**

```
Ora puoi selezionare e copiare più di una riga alla volta in Impostazioni → Diagnostica.
```

**Português**

```
Agora pode selecionar e copiar mais do que uma linha de cada vez em Definições → Diagnóstico.
```
