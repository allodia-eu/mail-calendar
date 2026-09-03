# A dismissed event editor stays dismissed

Platforms: macos, ios
Bump: patch

> Cancel and Save both closed the event editor and both immediately reopened it, on iPhone and
> iPad. `.sheet(item:)` keeps its content alive while the sheet animates away and that content is
> still writing (a text field commits, a focus state tears down), and the binding it was given read
> through to the value the sheet had been presented with. So a late write handed that value back to
> storage, the item was non-`nil` again, and the sheet presented itself a second time. The read
> still falls back, because SwiftUI evaluates the content closure on the frame where the item has
> just been cleared and force-unwrapping there traps; the write no longer does.
> `sheetItemBinding` holds both halves, and `SheetItemBindingTests` drives them without a sheet,
> since there is no Apple UI-test target to present one.

**English**

```
Closing an event with Cancel or Save no longer reopens it straight away.
```

**Nederlands**

```
Een afspraak sluiten met Annuleren of Bewaren opent hem niet langer meteen opnieuw.
```

**Deutsch**

```
Wer einen Termin mit Abbrechen oder Sichern schließt, bekommt ihn nicht mehr sofort wieder
geöffnet.
```

**Français**

```
Fermer un événement avec Annuler ou Enregistrer ne le rouvre plus aussitôt.
```

**Español**

```
Cerrar un evento con Cancelar o Guardar ya no vuelve a abrirlo de inmediato.
```

**Italiano**

```
Chiudere un evento con Annulla o Salva non lo riapre più subito.
```

**Português**

```
Fechar um evento com Cancelar ou Guardar deixa de o reabrir de imediato.
```
