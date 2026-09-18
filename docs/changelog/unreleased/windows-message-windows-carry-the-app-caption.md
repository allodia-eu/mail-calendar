# Windows message windows carry the app's own caption

Platforms: windows
Bump: patch

> Two halves of one inconsistency in the Windows caption. The mailbox already drew its own with the
> WinUI `TitleBar` control, but left the minimise / maximise / close buttons at the height the
> system gives them: 32 epx, hard against the top edge, in a caption that is 48. Their glyphs sat a
> third of a row above the icon and the app's name beside them, and a finger had a third less to aim
> at. `AppWindowTitleBar.PreferredHeightOption` is what moves them, and nothing in the XAML tree can
> see that it has not been set, because the buttons are drawn on a surface of the system's own. The
> reading and composer windows meanwhile drew no caption at all, so a message opened out of the
> mailbox wore a strip that followed neither the appearance setting nor the brand, next to a mailbox
> that did both. Both now go through one `WindowCaption.Extend`, and the height and the brand icon
> are read from the application's resources, so three windows cannot come up looking like three
> applications.

**English**

```
A message or draft opened in its own window now carries the app's own title bar, and the window buttons line up with it instead of sitting above it.
```

**Nederlands**

```
Een bericht of concept in een eigen venster heeft nu de titelbalk van de app zelf, en de vensterknoppen staan ermee op één lijn in plaats van erboven.
```

**Deutsch**

```
Eine Nachricht oder ein Entwurf in einem eigenen Fenster trägt jetzt die Titelleiste der App, und die Fensterschaltflächen liegen auf einer Linie mit ihr statt darüber.
```

**Français**

```
Un message ou un brouillon ouvert dans sa propre fenêtre porte désormais la barre de titre de l'application, et les boutons de fenêtre s'alignent dessus au lieu de la surplomber.
```

**Español**

```
Un mensaje o borrador abierto en su propia ventana ahora lleva la barra de título de la aplicación, y los botones de ventana quedan alineados con ella en vez de por encima.
```

**Italiano**

```
Un messaggio o una bozza aperti in una finestra propria ora hanno la barra del titolo dell'app, e i pulsanti della finestra sono allineati con essa invece che più in alto.
```

**Português**

```
Uma mensagem ou rascunho aberto numa janela própria passa a ter a barra de título da aplicação, e os botões da janela ficam alinhados com ela em vez de acima.
```
