# A subscription change that was refused says why

Platforms: android, linux, windows
Bump: patch

> `AllodiaSubscriptionRefusal` reaches a client as a code, which is what
> [`allodia_license/purchasing.md`](../../../allodia_license/purchasing.md) asks for, and until now
> the clients drawing the writes rendered the **exception** instead. UniFFI builds one out of the
> variant's own fields, so a refusal read `reason=NOT_SWITCHABLE` and every other failure, an
> unreachable service among them, read as a sentence with nothing after its colon. Five of the
> codes now carry a sentence of their own in all seven locales; `Unavailable` and a code a later
> service invents share the one that says only that nothing changed. All three clients switch on
> the code in a plain function with a test of its own, rather than in the drawing. The commonest of
> the four was the worst read: an unreachable service carries no message at all, so on Windows it
> reached the card as the exception's type name.

**English**

```
When a change to your subscription can’t be made, Settings now says why instead of showing a code.
```

**Nederlands**

```
Als een wijziging van je abonnement niet kan, zegt Instellingen nu waarom in plaats van een code te tonen.
```

**Deutsch**

```
Wenn eine Änderung an Ihrem Abonnement nicht möglich ist, nennen die Einstellungen jetzt den Grund, statt einen Code anzuzeigen.
```

**Français**

```
Lorsqu’une modification de votre abonnement est impossible, les réglages en donnent désormais la raison au lieu d’afficher un code.
```

**Español**

```
Cuando no se puede hacer un cambio en tu suscripción, los ajustes ahora dicen por qué en lugar de mostrar un código.
```

**Italiano**

```
Quando una modifica al tuo abbonamento non è possibile, le impostazioni ora ne indicano il motivo invece di mostrare un codice.
```

**Português**

```
Quando não é possível alterar a sua subscrição, as definições dizem agora porquê, em vez de mostrar um código.
```
