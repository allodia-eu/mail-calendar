# An invitation card follows the appearance setting

Platforms: windows
Bump: patch

> `Application.Current.Resources["…Brush"]` answers with the theme the **application** is in, which
> is the desktop's and is fixed before any window exists. The appearance choice is applied to the
> content root instead (it has to be: `Application.RequestedTheme` can only be set once, so pinning
> it there would make "Use system setting" unreachable without a restart). So on a desktop whose
> theme differed from the chosen appearance, every brush a code-built surface read that way came
> back inverted, and the invitation card's labels, organiser row, description and attendee tally
> were drawn in near-white on a light card. It rendered perfectly and was simply unreadable, which
> is why nothing caught it: the same text in the XAML beside it was right, because `{ThemeResource}`
> resolves against the element. `ThemePalette` now answers for the theme the element is actually in,
> and the card and the account-setup offer both repaint when the setting changes.

**English**

```
A meeting invitation is now readable when the app's appearance differs from the desktop's, instead of drawing its labels in near-white on a light card.
```

**Nederlands**

```
Een vergaderuitnodiging is nu leesbaar wanneer het uiterlijk van de app afwijkt van dat van het bureaublad, in plaats van de labels bijna wit op een lichte kaart te tonen.
```

**Deutsch**

```
Eine Besprechungseinladung ist jetzt auch dann lesbar, wenn das Erscheinungsbild der App von dem des Desktops abweicht, statt ihre Beschriftungen fast weiß auf heller Karte zu zeichnen.
```

**Français**

```
Une invitation à une réunion est désormais lisible lorsque l'apparence de l'application diffère de celle du bureau, au lieu d'afficher ses libellés en blanc cassé sur une carte claire.
```

**Español**

```
Una invitación de reunión ahora se lee bien cuando la apariencia de la aplicación no coincide con la del escritorio, en vez de mostrar sus etiquetas casi en blanco sobre una tarjeta clara.
```

**Italiano**

```
Un invito a una riunione ora è leggibile quando l'aspetto dell'app è diverso da quello del desktop, invece di mostrare le etichette quasi bianche su una scheda chiara.
```

**Português**

```
Um convite de reunião passa a ser legível quando o aspeto da aplicação difere do do ambiente de trabalho, em vez de mostrar as etiquetas quase a branco num cartão claro.
```
