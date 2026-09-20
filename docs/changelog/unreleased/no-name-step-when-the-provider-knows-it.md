# No "your name" step where the provider already knows the name

Platforms: all
Bump: patch

> The step was raised after every add, prefilled from `suggested_sender_name`, so an account
> whose provider keeps a display name arrived at a field filled in with a name the user had
> already chosen elsewhere and a button to confirm it. Every add route now asks the core
> `needs_sender_name` first, which adopts the provider's name where there is one and answers
> `false`. Adoption is the half a client may not do for itself: the `From` reads the stored
> name, so hiding the step without storing anything would send as a bare address. The adopted
> name is stored locally only, never pushed back to the provider it came from.

**English**

```
Connecting an account no longer asks for your name when your provider already knows it; the app takes the name you have there.
```

**Nederlands**

```
Bij het koppelen van een account vraagt de app niet meer naar je naam als je provider die al kent; ze neemt de naam over die je daar hebt.
```

**Deutsch**

```
Beim Verbinden eines Kontos fragt die App nicht mehr nach Ihrem Namen, wenn Ihr Anbieter ihn bereits kennt; sie übernimmt den Namen, den Sie dort haben.
```

**Français**

```
Lors de la connexion d'un compte, l'application ne demande plus votre nom si votre fournisseur le connaît déjà ; elle reprend celui que vous y avez.
```

**Español**

```
Al conectar una cuenta, la aplicación ya no pregunta tu nombre si tu proveedor ya lo conoce; toma el que tienes allí.
```

**Italiano**

```
Quando colleghi un account, l'app non chiede più il tuo nome se il tuo provider lo conosce già: prende quello che hai lì.
```

**Português**

```
Ao ligar uma conta, a aplicação deixa de pedir o seu nome quando o seu fornecedor já o conhece; passa a usar o nome que tem lá.
```
