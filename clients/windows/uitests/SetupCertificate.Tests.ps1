#!/usr/bin/env pwsh
# A calendar server whose certificate does not verify, set up as an account without mail
# (docs/certificate-exceptions.md, docs/accounts.md): the refusal shows the certificate, Connect
# stays inert until it is accepted, and accepting it connects the account.
#
# Why it is here and not in `Mailcal.Tests`: the core's half is unit-tested (the refusal carries the
# certificate; an acceptance is stored with the account), and the form's rules for the panel are
# WinUI. What neither can see is the refusal reaching the form at all on this route, which is how
# Android's panel was once written, tested and unreachable.
#
# The dataset is `harness-setup`: the first-run screen beside the harness. The server is the
# harness's own CalDAV and CardDAV behind an HTTPS listener on 12443 whose certificate no debug
# build is handed (docker/stalwart/front/nginx.conf). The account the suite sets up is removed at
# the end, which returns the window to the screen it started on.

. (Join-Path $PSScriptRoot 'harness-setup.ps1')

$CertificateLogin = 'alice@test.local'
$CertificateServer = 'https://localhost:12443'

<#
.SYNOPSIS
The SHA-256 of the certificate the untrusted listener serves, read off the listener itself.
.DESCRIPTION
From the socket rather than from a file: what the panel has to match is what the server presented,
and a fingerprint read from anywhere else proves only that two copies of one file agree.
#>
function Get-ListenerFingerprint {
  $tcp = [System.Net.Sockets.TcpClient]::new('127.0.0.1', 12443)
  try {
    $ssl = [System.Net.Security.SslStream]::new($tcp.GetStream(), $false, { $true })
    $ssl.AuthenticateAsClient('localhost')
    [System.Security.Cryptography.X509Certificates.X509Certificate2]::new($ssl.RemoteCertificate).GetCertHashString('SHA256')
  }
  finally { $tcp.Dispose() }
}

$Suite = @{
  Dataset = 'harness-setup'
  Env     = @{ MAILCAL_DEV_ACCOUNT = 'first-run' }
  Cases   = @(
    @{
      Name = 'the refused certificate is shown, fingerprint included, and Connect waits for the box'
      Body = {
        Open-ManualSetup -Address $CertificateLogin -Type 'DavChoice'
        Set-UiaText -Element (Find-UiaElement -AutomationId 'DavLogin' -Type 'Edit') -Text $CertificateLogin
        Set-UiaText -Element (Find-UiaElement -AutomationId 'DavPassword' -Type 'Edit') -Text 'harness-alice-pw'
        Set-UiaText -Element (Find-UiaElement -AutomationId 'DavCaldavUrl' -Type 'Edit') -Text $CertificateServer
        Set-UiaText -Element (Find-UiaElement -AutomationId 'DavCarddavUrl' -Type 'Edit') -Text $CertificateServer
        Invoke-UiaElement (Find-UiaElement -AutomationId 'ConnectButton' -Type 'Button')
        $claims = Wait-UiaElement -AutomationId 'CertificateClaims' -TimeoutSec 30
        Assert-True ($null -ne $claims) (
          'docs/certificate-exceptions.md rule 3: a calendar server refused on its certificate ' +
          'shows that certificate, for an account without mail as for a mailbox')
        $shown = ($claims.Current.Name -replace '[^0-9A-Fa-f]', '')
        $served = Get-ListenerFingerprint
        Assert-True ($shown.ToUpperInvariant().Contains($served)) (
          "rule 4: the panel's SHA-256 is the certificate the server presented ($served). " +
          "The panel reads: $($claims.Current.Name)")
        $connect = Find-UiaElement -AutomationId 'ConnectButton' -Type 'Button'
        Assert-True (-not $connect.Current.IsEnabled) (
          'rule 5: Connect stays inert until the box is ticked')
        $check = Find-UiaElement -AutomationId 'CertificateCheck'
        Set-UiaToggle $check -On
        Assert-True (Find-UiaElement -AutomationId 'ConnectButton' -Type 'Button').Current.IsEnabled (
          'ticking the box is what lets Connect go')
        Assert-Equal $CertificateServer (Get-UiaText (Find-UiaElement -AutomationId 'DavCaldavUrl' -Type 'Edit')) (
          'rule 6: the question is answered where it was asked, on the form as it was filled in')
      }
    }
    @{
      Name = 'accepting it connects the account, used for its calendar and contacts and no mail'
      Body = {
        Invoke-UiaElement (Find-UiaElement -AutomationId 'ConnectButton' -Type 'Button')
        Complete-SetupPrompts
        $dialog = Open-SettingsAccount -Address $CertificateLogin
        foreach ($use in 'Calendar', 'Contacts') {
          $toggle = Find-UiaElement -AutomationId "AccountUse_$use" -Root $dialog
          Assert-True ($null -ne $toggle -and (Get-UiaToggle $toggle)) (
            "the account connected through the accepted certificate and is used for its $use")
        }
        $mail = Find-UiaElement -AutomationId 'AccountUse_Mail' -Root $dialog
        Assert-True ($null -eq $mail -or -not (Get-UiaToggle $mail)) (
          'an account set up as calendar and contacts is used for no mail')
        Assert-True ($null -eq (Find-UiaElement -Name 'Sign in again' -Root $dialog)) (
          'and it is connected: nothing on its page asks to sign in again')
      }
    }
    @{
      Name = 'removing it returns the window to first-run setup'
      Body = { Remove-OnlyAccount -Address $CertificateLogin }
    }
  )
}
