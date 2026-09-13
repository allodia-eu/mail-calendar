# Newsletters reflow to the reading pane

Platforms: all
Bump: minor

> Half of real mail has no `@media` rules at all: a table pinned to 600px in the markup and again
> inline, its cells pinned to a third of that each. It is now reflowed to the pane rather than cut
> off at the right edge, in the **shared document** (`crates/mailcal-app/src/html/reflow.rs`), so
> it is one implementation and every client has it.
>
> The approach is the mail clients' own, the "munger" that came from AOSP Email through K-9 Mail
> and Thunderbird for Android and that Infomaniak's iOS client still runs: reflow before scaling
> anything, because a reflowed message is readable at the reader's own text size while a scaled one
> is a small picture of a message. Where ours differs is that it is CSS rather than script, so it
> needs neither a script in the message (`rendering-security.md` gates 1 and 2) nor the pane's
> width, which the core does not have. `max-width:100%` is unconditional because it can only
> shrink; clearing the fixed widths off a table's cells sits behind a media query whose breakpoint
> is the widest fixed width the message asks for **plus the page's own inset on both edges**, so the
> engine re-decides on every resize with nothing re-rendered. Two traps have seeded fixtures because
> each is the other's regression test: the selector asks for a width attribute that is not a
> percentage, or a full-bleed `<table width="100%" bgcolor>` band would stop spanning the pane; and
> the breakpoint carries the padding, or there is a 28px band of pane widths, which a 13-inch iPad
> lands in, where a message is clipped and nothing fires.

**English**

```
A newsletter written for a wider screen now reflows to fit the reading pane instead of running off the right edge, at whatever width you read it.
```

**Nederlands**

```
Een nieuwsbrief die voor een breder scherm is gemaakt, past zich nu aan het leesvenster aan in plaats van weg te vallen aan de rechterkant, op elke breedte waarop u leest.
```

**Deutsch**

```
Ein Newsletter, der für einen breiteren Bildschirm gemacht wurde, passt sich jetzt dem Lesebereich an, statt am rechten Rand abgeschnitten zu werden, und das bei jeder Lesebreite.
```

**Français**

```
Une lettre d'information conçue pour un écran plus large s'adapte désormais au volet de lecture au lieu de déborder à droite, quelle que soit la largeur à laquelle vous la lisez.
```

**Español**

```
Un boletín pensado para una pantalla más ancha ahora se adapta al panel de lectura en lugar de salirse por la derecha, sea cual sea el ancho con el que lo lea.
```

**Italiano**

```
Una newsletter pensata per uno schermo più largo ora si adatta al riquadro di lettura invece di uscire dal bordo destro, a qualunque larghezza la si legga.
```

**Português**

```
Uma newsletter feita para um ecrã mais largo passa a adaptar-se ao painel de leitura em vez de sair pela margem direita, seja qual for a largura a que a lê.
```
