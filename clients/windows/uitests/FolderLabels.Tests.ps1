# A known folder is called what WE call it, not what the server calls it
# (docs/folder-pane.md rule 12).
#
# THE FIXTURE is the harness account, and it has to be: the showcase seed already names its folders
# "Inbox" and "Sent", so it cannot tell a working rename from no rename at all. A real IMAP server
# names them `INBOX` (shouting, the one name the protocol mandates), `Sent Items`, `Junk Mail`,
# `Deleted Items`, which is what the user saw, and what this asserts is gone.
#
# The other half matters just as much: a CUSTOM folder must keep its own name. Mapping by name
# instead of by role would either miss these renames or, worse, rename somebody's own folder.

# Every sidebar row's name, read exactly as drawn.
#
# Not Find-UiaElement: its -Name match is case-INSENSITIVE, so asking whether a row called `INBOX`
# exists happily answers yes for `Inbox`, and the whole point here is the casing. The first draft
# of this file passed that way for the wrong reason.
function Get-SidebarNames {
  Get-UiaTree |
    Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::ListItem } |
    ForEach-Object { $_.Current.Name }
}

$Suite = @{
  Dataset = 'harness'
  Cases   = @(
    @{
      Name = 'the server names of known folders are replaced by our own'
      Body = {
        $names = @(Get-SidebarNames)
        foreach ($ours in @('Inbox', 'Drafts', 'Sent', 'Junk', 'Trash')) {
          Assert-True ($names -ccontains $ours) "the pane calls it '$ours'"
        }
        # `-cnotcontains`: case-sensitive, because `INBOX` versus `Inbox` IS the bug.
        foreach ($theirs in @('INBOX', 'Sent Items', 'Junk Mail', 'Deleted Items')) {
          Assert-True ($names -cnotcontains $theirs) `
            "the server's '$theirs' is not what the user reads"
        }
      }
    },
    @{
      Name = 'a folder the user made keeps the name the user gave it'
      Body = {
        # The seeded custom folders. If these ever start reading "Folder" or an app word, the
        # mapping has stopped keying on the role, which is the failure that renames real mail.
        $names = @(Get-SidebarNames)
        foreach ($theirs in @('Projects', 'QResync', 'Idle')) {
          Assert-True ($names -ccontains $theirs) "'$theirs' is the user's folder and keeps its name"
        }
      }
    }
  )
}
