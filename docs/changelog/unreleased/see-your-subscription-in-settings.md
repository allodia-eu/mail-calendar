# See your subscription in Settings on macOS, iOS and Android

Platforms: macos, ios, android
Bump: minor

> The first screens [`allodia_license/purchasing.md`](../../../allodia_license/purchasing.md) has
> had. The core already answered the whole section in one read and nothing called it; Settings →
> Allodia account now does, below the account box and only while somebody is signed in. What it
> draws is what the core answered: `actions` decides which buttons exist, `orderAllodiaOffers`
> decides which periods appear and in what order, and a store's price is drawn rather than parsed,
> which both stores require and which the core has no locale data to do anyway. Three rules the
> contract states and a screen could get backwards are unit-tested rather than left to the drawing:
> a failed charge the store is still retrying is not a lapse, a subscription nobody will be charged
> for again runs out rather than renews, and every biller charging one account is named, or a
> warning about paying twice names one of the two. `AllodiaPurchases` is finally constructed and
> its listener started at connect on both platforms, so a renewal, a refund and an approval granted
> elsewhere reach the redemption pass. Android draws one card for two flavours and each gets the
> shop its flavour supplies: `play` sells through Play, `foss` opens Allodia's own checkout in a
> browser. ⚠️ Nothing here has taken a real payment on either platform.

**English**

```
Settings now shows your subscription: what you're being charged, by whom, when it renews, and where to change or cancel it.
```

**Nederlands**

```
Instellingen laat nu je abonnement zien: wat je betaalt, aan wie, wanneer het wordt verlengd, en waar je het kunt wijzigen of opzeggen.
```

**Deutsch**

```
Die Einstellungen zeigen jetzt Ihr Abonnement: was Ihnen berechnet wird, von wem, wann es sich verlängert und wo Sie es ändern oder kündigen können.
```

**Français**

```
Les réglages affichent désormais votre abonnement : ce qui vous est facturé, par qui, quand il se renouvelle et où le modifier ou le résilier.
```

**Español**

```
Los ajustes ahora muestran tu suscripción: qué se te cobra, quién lo cobra, cuándo se renueva y dónde cambiarla o cancelarla.
```

**Italiano**

```
Le impostazioni ora mostrano il tuo abbonamento: che cosa ti viene addebitato, da chi, quando si rinnova e dove modificarlo o disdirlo.
```

**Português**

```
As definições mostram agora a sua subscrição: o que lhe é cobrado, por quem, quando se renova e onde a pode alterar ou cancelar.
```
