# Unread mail has to LOOK unread, the subject and the sender both, on a flat row and on a
# conversation header alike (AGENTS.md; the Android twin is UnreadEmphasisTest.kt).
#
# This is the suite that would have caught the bug it was written for. The Windows conversation
# header bolded nothing on unread mail, not because the binding was wrong, `TitleWeight` was bound
# correctly the whole time, but because `MailRow.Unread` was never assigned for a thread row, so
# the binding read a field that was permanently false. Nothing without a rendered window can see
# that: the XAML compiles, the property exists, the projection returns a well-formed row.
#
# Reading the weight is exact rather than visual: a WinUI TextBlock exposes TextPattern, so
# `Get-UiaFontWeight` returns 400 (Normal) or 600 (SemiBold) off the text as drawn.
#
# THE FIXTURE is the `en` showcase seed (crates/mailcal-bindings/src/showcase_data/en.rs), pinned by
# the runner to -Locale en. The harness mailbox cannot stand in for it: every seeded message there
# is READ, which is exactly why a bug about unread rows survived so long in a repo with a mail
# harness. If a case below stops finding its row, that seed changed, fix the constant, and check
# the message is still unread (no `.seen()`) before assuming the app broke.

$Unread = [pscustomobject]@{ Title = 'Welcome to Allodia Mail*'; Sender = 'Allodia' }
$Read = [pscustomobject]@{ Title = 'Your June usage report'; Sender = 'Example Cloud' }
# Three messages, two of them unread, the conversation the Windows projection could never bold.
$UnreadThread = [pscustomobject]@{ Title = 'Re: Q3 launch*'; Sender = 'Tom de Vries' }

$SemiBold = 600
$Normal = 400

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'an unread message bolds its subject'
      Body = {
        $row = Get-MailRowByTitle $Unread.Title
        Assert-Equal $SemiBold (Get-RowTextWeight -Row $row -Text $Unread.Title) `
          'the subject of an unread message is drawn SemiBold'
      }
    },
    @{
      Name = 'an unread message bolds its sender too'
      Body = {
        $row = Get-MailRowByTitle $Unread.Title
        Assert-Equal $SemiBold (Get-RowTextWeight -Row $row -Text $Unread.Sender) `
          'the sender moves with the subject, the accent dot alone is a small target for the eye, and it is the sender that says whether an unread row is worth opening'
      }
    },
    @{
      Name = 'a read message leaves subject and sender alone'
      Body = {
        $row = Get-MailRowByTitle $Read.Title
        Assert-Equal $Normal (Get-RowTextWeight -Row $row -Text $Read.Title) `
          'a read subject stays Normal'
        Assert-Equal $Normal (Get-RowTextWeight -Row $row -Text $Read.Sender) `
          'a read sender stays Normal, so bolding unread rows cannot make the read ones louder too'
      }
    },
    @{
      Name = 'unread and read are actually distinguishable'
      Body = {
        # The assertion the Kotlin twin exists for. Returning one weight from both arms compiles,
        # renders, and looks exactly like a feature nobody implemented, every case above would
        # still pass against a pair of constants that happened to match. This one cannot.
        $unreadRow = Get-MailRowByTitle $Unread.Title
        $readRow = Get-MailRowByTitle $Read.Title
        Assert-GreaterThan (Get-RowTextWeight -Row $readRow -Text $Read.Title) `
          (Get-RowTextWeight -Row $unreadRow -Text $Unread.Title) `
          'an unread subject must be HEAVIER than a read one, not merely equal to a constant'
        Assert-GreaterThan (Get-RowTextWeight -Row $readRow -Text $Read.Sender) `
          (Get-RowTextWeight -Row $unreadRow -Text $Unread.Sender) `
          'and so must an unread sender'
      }
    },
    @{
      Name = 'an unread conversation header bolds subject and sender'
      Body = {
        # THE REGRESSION. A conversation is unread while anything in it is; the header is a summary,
        # and it may not read as settled while it hides an unread reply. Before the fix this row
        # carried no unread flag at all, so both of these read 400.
        $row = Get-MailRowByTitle $UnreadThread.Title
        Assert-Equal $SemiBold (Get-RowTextWeight -Row $row -Text $UnreadThread.Title) `
          'a conversation holding unread mail bolds its subject'
        Assert-Equal $SemiBold (Get-RowTextWeight -Row $row -Text $UnreadThread.Sender) `
          'and its latest sender'
      }
    }
  )
}
