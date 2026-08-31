# The folder pane as the user actually gets it (docs/folder-pane.md).
#
# Everything here is invisible to Mailcal.Tests, which cannot link WinUI: the shaping rules are
# pinned there against SidebarTree, but whether the shape reaches a rendered NavigationView, with
# a name a screen reader can read and a badge that is really on screen, only a running window can
# answer.
#
# It exists because this change broke exactly that and nothing else noticed. Giving the row a Grid
# for its Content (to make room for the trailing count) silently took away its automation Name: a
# NavigationViewItem derives its name from Content, and a Grid has no text. On screen it looked
# perfect. To a screen reader every folder row was nameless, and to automation the sidebar had
# stopped existing.
#
# THE FIXTURE is the `en` showcase seed (crates/mailcal-bindings/src/showcase_data/en.rs), pinned by
# the runner to -Locale en: TWO accounts, which is what makes the independence rules testable at
# all, one account's tree shutting while the other stays open is not a thing one account can show.
# Its unread counts are derived from its own seeded messages (`count_unread`), so they move with the
# seed rather than being written down twice. The harness mailbox cannot stand in: every message it
# seeds is READ, so every badge there is legitimately absent.

$First = 'eva.jansen@example.com'   # Inbox (4 unread), Drafts, Sent, Archive
$Second = 'eva@northwind.example'   # Inbox (1 unread), Sent, seeded EMPTY, and load-bearing below

# A sidebar row by its spoken name, what a screen reader announces, and what every automation
# lookup matches on. `Find-UiaElement -Name` is an EXACT match, not a wildcard.
function Get-SidebarRow {
  param([Parameter(Mandatory)] [string] $Name)
  Find-UiaElement -Root (Get-MailcalWindow) -Name $Name -Type 'ListItem'
}

# Every row in the pane with that name, there are two "Inbox" rows, one per account.
function Get-SidebarRows {
  param([Parameter(Mandatory)] [string] $Name)
  @(Find-UiaElements -Root (Get-MailcalWindow) -Name $Name -Type 'ListItem')
}

# The unread sentence drawn on a row, or $null when the row carries no badge. Read off the row's
# own subtree, so "no badge" is genuinely no element rather than an element reading zero.
#
# It stops at a nested ListItem, and that is load-bearing rather than tidy: an ACCOUNT row's
# subtree contains its folder rows, so a plain descendant walk reads the Inbox's "4 unread" off the
# account and reports a roll-up that is not on screen, a false failure of rule 8 the first draft
# of this file duly produced.
function Get-RowUnread {
  param([Parameter(Mandatory)] [object] $Row)
  $own = @(
    $Row.FindAll($script:UiaChildren, $script:UiaAny) |
      Where-Object { $_.Current.ControlType -ne [System.Windows.Automation.ControlType]::ListItem } |
      ForEach-Object { Get-UiaTree -Root $_ }
  )
  $text = @($own | Where-Object { $_.Current.Name -match 'unread$' })
  if ($text.Count -eq 0) { return $null }
  $text[0].Current.Name
}

# Whether a row is actually ON SCREEN.
#
# Not "does the element exist": WinUI keeps a collapsed NavigationViewItem's children realised, so
# a shut account's folders are still in the automation tree, findable by name, and reading exactly
# like an open tree to any test that only looks them up. `IsOffscreen` is what separates the two,
# and asserting on presence instead is how this file would have passed over a pane that never
# collapsed at all.
function Test-RowOnScreen {
  param([Parameter(Mandatory)] [string] $Name)
  $row = Get-SidebarRow -Name $Name
  ($null -ne $row) -and (-not $row.Current.IsOffscreen)
}

# What the mail list calls the scope it is showing, the one place on screen that says which
# mailbox the pane actually opened. A TextBlock's automation Name IS its text, so this reads the
# rendered string rather than the property behind it.
function Get-MailboxHeader {
  (Find-UiaElement -Root (Get-MailcalWindow) -AutomationId 'MailboxHeader').Current.Name
}

# Opens a sidebar row and waits for the list to answer. A dispatch is handled on the core's own
# runtime and its snapshot arrives back asynchronously, so the header does not change with the
# click, poll for it rather than sleeping a guessed interval.
function Open-SidebarRow {
  param(
    [Parameter(Mandatory)] [object] $Row,
    [Parameter(Mandatory)] [string] $Expect,
    [int] $TimeoutSec = 10)
  Invoke-UiaElement $Row -SettleMs 200
  $deadline = (Get-Date).AddSeconds($TimeoutSec)
  while ((Get-Date) -lt $deadline) {
    if ((Get-MailboxHeader) -eq $Expect) { break }
    Start-Sleep -Milliseconds 200
  }
  # Both snapshots have landed by now; give the list its own beat to reconcile behind the header.
  Start-Sleep -Milliseconds 400
}

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'every row in the pane announces its own name'
      Body = {
        # The regression above, stated as the thing it costs: a row with no name is a row a
        # screen reader cannot read and automation cannot find, however right it looks.
        foreach ($name in @('All Inboxes', $First, $Second, 'Drafts', 'Archive')) {
          Assert-True ($null -ne (Get-SidebarRow -Name $name)) "the pane has a row named '$name'"
        }
        Assert-Equal 2 (Get-SidebarRows -Name 'Inbox').Count 'both accounts show their Inbox'
      }
    },
    @{
      Name = 'both accounts show their folders while neither is selected'
      Body = {
        # The pane opens on All Inboxes, no account selected, and BOTH trees are on screen.
        # Under the old rule (expansion WAS selection) at most one account could ever have
        # folders showing, and on this screen none of them would.
        Assert-Equal 2 (Get-SidebarRows -Name 'Sent').Count `
          'each account contributes its own Sent row'
        Assert-True ($null -ne (Get-SidebarRow -Name 'Drafts')) `
          'the first account, which nothing has selected, still shows its folders'
      }
    },
    @{
      Name = 'the unread count reads as a sentence, on the folder and on All Inboxes'
      Body = {
        # "Inbox, 4" read aloud is a position in a list; the badge carries the words.
        $inbox = (Get-SidebarRows -Name 'Inbox')[0]
        Assert-Equal '4 unread' (Get-RowUnread -Row $inbox) 'the Inbox badge names what it counts'
        # And the roll-up is the sum across BOTH accounts (4 + 1), not the selected one's.
        $unified = Get-SidebarRow -Name 'All Inboxes'
        Assert-Equal '5 unread' (Get-RowUnread -Row $unified) `
          'All Inboxes sums every account inbox'
      }
    },
    @{
      Name = 'a folder with nothing unread carries no badge at all'
      Body = {
        # Zero is not drawn, and that deliberately covers "this provider reports no count" too,
        # so a 0 here would be the app claiming it had looked (rule 6).
        $drafts = Get-SidebarRow -Name 'Drafts'
        Assert-True ($null -eq (Get-RowUnread -Row $drafts)) `
          'a folder with nothing waiting draws no badge'
        # The account row stays bare too, however much is unread beneath it (rule 8).
        Assert-True ($null -eq (Get-RowUnread -Row (Get-SidebarRow -Name $First))) `
          'the account row carries no roll-up of its own'
      }
    },
    @{
      Name = 'shutting one account leaves the other one open'
      Body = {
        $account = Get-SidebarRow -Name $First
        $pattern = $account.GetCurrentPattern(
          [System.Windows.Automation.ExpandCollapsePattern]::Pattern)
        Assert-Equal 'Expanded' $pattern.Current.ExpandCollapseState.ToString() `
          'an account nobody has shut opens expanded'

        $pattern.Collapse()
        Start-Sleep -Milliseconds 800

        # Its own folders go off screen…
        Assert-True (-not (Test-RowOnScreen -Name 'Drafts')) `
          'the shut account takes its folders with it'
        # …and the other account's stay, which is the whole rule: expansion is per account and
        # nothing about it is shared with the selection.
        Assert-True (Test-RowOnScreen -Name $Second) 'the other account is still there'
        Assert-True (Test-RowOnScreen -Name 'All Inboxes') 'and the unified row is untouched'

        # Put it back, so the next run starts from the seeded default.
        $pattern.Expand()
        Start-Sleep -Milliseconds 800
        Assert-True (Test-RowOnScreen -Name 'Drafts') 'reopening restores the tree'
      }
    },
    @{
      Name = 'a folder opens even though no account is selected'
      Body = {
        # The bug this is here for: the pane opens on All Inboxes, and from there clicking a
        # folder did NOTHING AT ALL, no list change, no header change, and the highlight snapped
        # straight back to All Inboxes. The core's unified scope ignores a folder outright (there
        # is no account for the key to belong to), so the shell has to name the account alongside it.
        Assert-Equal 'All Inboxes' (Get-MailboxHeader) 'the app opens on the unified list'

        # Drafts belongs to the first account and to no other, so opening it needs that account
        # selected, which nothing has done.
        Open-SidebarRow -Row (Get-SidebarRow -Name 'Drafts') -Expect 'Drafts'
        Assert-Equal 'Drafts' (Get-MailboxHeader) 'clicking Drafts opens Drafts'
        Assert-Equal 1 @(Get-MailRows).Count 'and the list holds that account''s one draft'
      }
    },
    @{
      Name = 'a folder key opens the mailbox of the account whose tree it is in'
      Body = {
        # Both accounts have a folder keyed `sent`, every provider names its folders the same
        # way, which is why rule 14 exists. The two are told apart only by the account, so the
        # same word in the pane has to open two different mailboxes. The second account's Sent is
        # EMPTY in this seed and the first account's is not, which is what makes the difference
        # visible rather than merely asserted.
        $sent = Get-SidebarRows -Name 'Sent'
        Assert-Equal 2 $sent.Count 'both accounts contribute a Sent row'

        Open-SidebarRow -Row $sent[1] -Expect 'Sent'
        Assert-Equal 'Sent' (Get-MailboxHeader) 'the second account''s Sent opens'
        Assert-Equal 0 @(Get-MailRows).Count 'and it is empty, as that account''s seed is'

        Open-SidebarRow -Row $sent[0] -Expect 'Sent'
        Assert-GreaterThan 0 @(Get-MailRows).Count `
          'the first account''s Sent, under the same folder key, holds its own mail'

        # Back to the unified list, so the suite leaves the app where it found it.
        Open-SidebarRow -Row (Get-SidebarRow -Name 'All Inboxes') -Expect 'All Inboxes'
        Assert-Equal 'All Inboxes' (Get-MailboxHeader) 'and All Inboxes takes it back'
      }
    }
  )
}
