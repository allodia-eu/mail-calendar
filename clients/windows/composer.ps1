#!/usr/bin/env pwsh
# Reading and setting the composer's DOCUMENT, which UI Automation cannot see.
#
# Dot-source it; it defines functions and does nothing on its own:
#
#     ./control.ps1 home -DevToolsPort 9222       # or ./showcase.ps1 … -DevToolsPort 9222
#     . ./composer.ps1
#     Reset-ComposerBody '<p>Before |after</p>'   # the caret goes where the | is
#     winapp ui send-keys ctrl+v -a Mailcal --via send-input
#     Get-ComposerHtml                            # what the paste left in the message
#
# The editor is a page in a WebView2, so UI Automation reaches it as one group named "Write your
# message": whether a paste kept its bold, or where a picture landed, is not in that tree at all.
# The page is reached instead through the DevTools protocol, which the WebView2 runtime opens when
# the app is launched with `--remote-debugging-port`. The client never asks for it; the launch
# scripts pass it through WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS for that one process and remove
# it again, because the variable is process-wide and every later launch would open the port too.
#
# Only a launch that asked for the port has one, so a script here that finds nothing listening is
# reading a window launched without it, not a composer that is missing.

<#
.SYNOPSIS
Evaluates -Expression in the composer's page and returns its value.
.DESCRIPTION
The page is the one holding `#editor`. A promise is awaited, and the value comes back by value, so
an expression should end in something JSON can carry: a string, a number, an array or an object.
A script error is thrown, never returned as a result.
#>
function Invoke-ComposerScript {
  param([Parameter(Mandatory, Position = 0)] [string] $Expression, [int] $Port = 9222)
  try {
    $targets = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json" -TimeoutSec 5
  }
  catch {
    throw "nothing answers DevTools on port $Port; launch the app with -DevToolsPort $Port (control.ps1 or showcase.ps1)"
  }
  foreach ($target in @($targets | Where-Object type -eq 'page')) {
    $hasEditor = Invoke-DevToolsEvaluate $target.webSocketDebuggerUrl "!!document.getElementById('editor')"
    if ($hasEditor -eq $true) { return Invoke-DevToolsEvaluate $target.webSocketDebuggerUrl $Expression }
  }
  throw 'no page with #editor is open; is a composer on screen?'
}

function Invoke-DevToolsEvaluate {
  param([string] $Url, [string] $Expression)
  $socket = [System.Net.WebSockets.ClientWebSocket]::new()
  $cancel = [System.Threading.CancellationToken]::None
  try {
    [void]$socket.ConnectAsync([Uri] $Url, $cancel).GetAwaiter().GetResult()
    $request = @{
      id = 1
      method = 'Runtime.evaluate'
      params = @{ expression = $Expression; returnByValue = $true; awaitPromise = $true }
    } | ConvertTo-Json -Compress -Depth 5
    $bytes = [Text.Encoding]::UTF8.GetBytes($request)
    [void]$socket.SendAsync([ArraySegment[byte]]::new($bytes), 'Text', $true, $cancel).GetAwaiter().GetResult()
    # One reply, possibly in several frames: a message body is easily larger than one buffer.
    $buffer = [byte[]]::new(65536)
    $reply = [IO.MemoryStream]::new()
    do {
      $frame = $socket.ReceiveAsync([ArraySegment[byte]]::new($buffer), $cancel).GetAwaiter().GetResult()
      $reply.Write($buffer, 0, $frame.Count)
    } until ($frame.EndOfMessage)
    $answer = [Text.Encoding]::UTF8.GetString($reply.ToArray()) | ConvertFrom-Json -Depth 50
    if ($answer.result.exceptionDetails) {
      throw "the page threw: $($answer.result.exceptionDetails.exception.description)"
    }
    $answer.result.result.value
  }
  finally {
    $socket.Dispose()
  }
}

<#
.SYNOPSIS
The message body's markup as the editor holds it now.
.DESCRIPTION
A pasted or dropped picture is carried as a `data:` URI, which would bury the markup around it, so
each one is shortened to its length.
#>
function Get-ComposerHtml {
  Invoke-ComposerScript @'
document.getElementById('editor').innerHTML.replace(/base64,[A-Za-z0-9+\/=]{40,}/g, (m) => `base64,<${m.length} chars>`)
'@
}

<#
.SYNOPSIS
Replaces the message body with -Html and puts the caret where its `|` was.
.DESCRIPTION
For a test that needs a known starting point, a paste into an empty draft or into the middle of a
sentence, rather than whatever the composer opened with. It writes the DOM directly, so it is not
an edit the user could undo, and the draft is not saved by it.
#>
function Reset-ComposerBody {
  param([Parameter(Mandatory, Position = 0)] [string] $Html)
  if (($Html.ToCharArray() | Where-Object { $_ -eq '|' }).Count -ne 1) {
    throw "-Html needs exactly one | to say where the caret goes: $Html"
  }
  $json = $Html | ConvertTo-Json -Compress
  # The | becomes a character no message holds, which is then found and taken out of its text node:
  # the caret sits INSIDE one run of text, as it does after typing, rather than between two nodes,
  # where the next keystroke would lose the space on one side of it.
  Invoke-ComposerScript @"
(() => {
  const editor = document.getElementById('editor');
  editor.innerHTML = $json.replace('|', '⁣');
  const walker = document.createTreeWalker(editor, NodeFilter.SHOW_TEXT);
  let node;
  while ((node = walker.nextNode()) && !node.data.includes('⁣')) {}
  const range = document.createRange();
  const offset = node.data.indexOf('⁣');
  node.deleteData(offset, 1);
  range.setStart(node, offset);
  range.collapse(true);
  const selection = getSelection();
  selection.removeAllRanges();
  selection.addRange(range);
  editor.focus();
  return editor.innerHTML;
})()
"@
}
