# See your subscription in Settings on macOS and iOS

Platforms: macos, ios
Bump: minor

> The first screen [`allodia_license/purchasing.md`](../../../allodia_license/purchasing.md) has
> had. The core already answered the whole section in one read and nothing called it; Settings →
> Allodia account now does, below the account box and only while somebody is signed in. What it
> draws is what the core answered: `actions` decides which buttons exist, `orderAllodiaOffers`
> decides which periods appear and in what order, and a store's price is drawn rather than parsed,
> which Apple requires and which the core has no locale data to do anyway. Three rules the contract
> states and a screen could get backwards are unit-tested rather than left to the drawing: a failed
> charge the store is still retrying is not a lapse, a subscription nobody will be charged for
> again runs out rather than renews, and every biller charging one account is named, or a warning
> about paying twice names one of the two. `AllodiaPurchases` is finally constructed and its
> listener started at connect, so a renewal, a refund and an Ask to Buy approved elsewhere reach
> the redemption pass. ⚠️ It does not ship yet: the sovereignty carve-out for a store dispatch is
> still undecided and the published policy mirror still lags, and either alone forbids it.

**English**

```
Settings now shows your subscription: what you're being charged, by whom, and when it renews, with a link to the store's own page to change or cancel it.
```

**Nederlands**

```
Instellingen laat nu je abonnement zien: wat je betaalt, aan wie, en wanneer het wordt verlengd, met een link naar de pagina van de store om het te wijzigen of op te zeggen.
```

**Deutsch**

```
Die Einstellungen zeigen jetzt Ihr Abonnement: was Ihnen berechnet wird, von wem, und wann es sich verlängert, mit einem Link zur Seite des Stores, um es zu ändern oder zu kündigen.
```

**Français**

```
Les réglages affichent désormais votre abonnement : ce qui vous est facturé, par qui, et quand il se renouvelle, avec un lien vers la page du store pour le modifier ou le résilier.
```

**Español**

```
Los ajustes ahora muestran tu suscripción: qué se te cobra, quién lo cobra y cuándo se renueva, con un enlace a la página de la tienda para cambiarla o cancelarla.
```

**Italiano**

```
Le impostazioni ora mostrano il tuo abbonamento: che cosa ti viene addebitato, da chi e quando si rinnova, con un link alla pagina dello store per modificarlo o disdirlo.
```

**Português**

```
As definições mostram agora a sua subscrição: o que lhe é cobrado, por quem e quando se renova, com uma ligação para a página da loja para a alterar ou cancelar.
```
