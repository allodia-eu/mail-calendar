# Collapsing the sidebar keeps your folder trees

Platforms: windows
Bump: patch

> A NavigationView shuts every hierarchical item on its way to the icon strip, and reports it
> through the same two-way `IsExpanded` binding a chevron click writes to. The shell could not tell
> the two apart, so it told the core, which persisted it: the pane came back with every account
> shut, and stayed that way across launches, because what is stored is the set of accounts the user
> collapsed (`docs/folder-pane.md`, rules 2 to 4).
>
> The discriminator is the pane itself: a write that arrives while `IsPaneOpen` is false is the
> pane's, not the user's, and a compact pane has no chevron to click, so nothing real is lost by
> ignoring every one of them. Reopening then re-asserts the trees from the core, since the
> framework does not put back what it shut.
>
> Caught by the actions bar's new case, which collapses the pane to prove New Mail survives it, and
> left the harness account shut for every later suite. `Mailcal.Tests` cannot see it: the binding
> that misreports is WinUI's, so the regression test is in `uitests/FolderPane.Tests.ps1`.

**English**

```
Collapsing the folder pane no longer shuts every account's folders for good: the trees you left open are open again when you bring the pane back.
```

**Nederlands**

```
Het inklappen van het mappenpaneel sluit de mappen van je accounts niet langer voorgoed: de mappen die je open had staan weer open zodra je het paneel terughaalt.
```

**Deutsch**

```
Das Einklappen der Ordnerleiste schließt die Ordner Ihrer Konten nicht mehr dauerhaft: Was Sie offen gelassen haben, ist wieder offen, sobald Sie die Leiste zurückholen.
```

**Français**

```
Réduire le volet des dossiers ne referme plus définitivement les dossiers de vos comptes : ce que vous aviez laissé ouvert l'est de nouveau dès que le volet revient.
```

**Español**

```
Contraer el panel de carpetas ya no cierra para siempre las carpetas de tus cuentas: lo que dejaste abierto vuelve a estarlo en cuanto recuperas el panel.
```

**Italiano**

```
Comprimere il pannello delle cartelle non chiude più per sempre le cartelle dei tuoi account: quello che avevi lasciato aperto è di nuovo aperto quando riporti il pannello.
```

**Português**

```
Recolher o painel de pastas já não fecha para sempre as pastas das suas contas: o que deixou aberto volta a estar aberto assim que traz o painel de volta.
```
