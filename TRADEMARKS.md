# Trademark policy

**The licence covers the code. It does not license the name.** [`LICENSE`](LICENSE) grants you the
freedoms the GPL grants, and none of them is a right to call your build Allodia Mail & Calendar.
That separation is deliberate, and it is the ordinary one: the GPL is a copyright licence, and
trademark law is what stops one program being passed off as another.

## The marks

**Allodia** and **Allodia Mail & Calendar**, together with the Allodia logo, wordmark and mascot,
are trademarks of Allodia.

## What you may do

Everything the licence says, without asking:

- build, run and modify the source, for any purpose including a commercial one;
- publish your changes, fork the project, and **distribute or sell your build**: to a store, a
  distribution, or anyone else.

You do not need permission, and there is no separate agreement to sign.

## Ship it under your own name

A build carries an identity (a name, an application id, an icon), and yours must be yours.
This is one file: `branding/allodia.env` sets the Allodia identity, and **without it a build is
already neutral**: the name, the application id, everything named after that id (OAuth redirect
schemes, keychain and app groups, the data directory), the icon and the welcome art. So a fork is
rebranded by omission rather than by a rename chased through five clients. Point the same
generators at your own source image and the icons are yours too.

It is not yet complete, and [`docs/branding.md`](docs/branding.md) → **Known gaps** is the list:
a few places still name Allodia in an unbranded build: the MCP relay's binary name, the Linux
AppStream publisher block, one catalog string. They are being closed. None of them is a reason you
cannot ship; if one is in your way, say so and it moves up.

So: do not name your build Allodia Mail & Calendar, do not ship the Allodia logo, wordmark or
mascot, and do not present it in a way that suggests Allodia published it or endorses it.

## Saying what it is

Naming the project to describe your own is fine and needs no permission: "a fork of Allodia Mail
& Calendar", "based on Allodia Mail & Calendar", "compatible with". Use the name to refer to this
project, not as the name of yours.

## If you are unsure

Ask: **info@allodia.eu**. A use that is honest about who made what is very unlikely to be a
problem.
