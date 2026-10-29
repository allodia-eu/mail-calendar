# Remove an account without waiting for the store to be compacted

Platforms: all
Bump: patch

> Removing an account compacted the whole store (`VACUUM`) before the removal returned, and on
> Apple that call ran on the main thread: a 1.5 GB store held the window for about 17 seconds.
> The compaction now runs after the removal, unawaited, and Apple calls the removal off the main
> actor.

**English**

```
Removing an account now takes it off screen straight away; the space it used is freed afterwards, in the background.
```

**Nederlands**

```
Een account verwijderen haalt het nu meteen uit beeld; de ruimte die het innam wordt daarna op de achtergrond vrijgemaakt.
```

**Deutsch**

```
Ein Konto zu entfernen nimmt es jetzt sofort aus der Ansicht; der Speicher, den es belegte, wird danach im Hintergrund freigegeben.
```

**Français**

```
Supprimer un compte le retire désormais aussitôt de l'écran ; l'espace qu'il occupait est libéré ensuite, en arrière-plan.
```

**Español**

```
Eliminar una cuenta la quita ahora de la pantalla al instante; el espacio que ocupaba se libera después, en segundo plano.
```

**Italiano**

```
Rimuovere un account ora lo toglie subito dallo schermo; lo spazio che occupava viene liberato dopo, in background.
```

**Português**

```
Remover uma conta retira-a agora do ecrã de imediato; o espaço que ocupava é libertado depois, em segundo plano.
```
