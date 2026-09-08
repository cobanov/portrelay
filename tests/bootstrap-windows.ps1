# Run only on a disposable GitHub Actions Windows runner, never a workstation.
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Requires a disposable GitHub Actions runner.' }
$bootstrap = Join-Path $PSScriptRoot '../web/install.ps1'
$app = Join-Path $env:ProgramFiles 'PortRelay'
if (Test-Path $app) { throw 'Runner already has PortRelay installed.' }

function Assert-Fails([scriptblock]$Action, [string]$Message) {
    try { & $Action } catch {
        if ($_.Exception.Message -like "*$Message*") { return }
        throw
    }
    throw "Expected failure containing: $Message"
}

# Hosted runners use Server. Check that the real guard rejects it, then supply
# only the Client platform fixture for bootstrap tests. This is not Windows 11
# desktop/UAC or USB validation; those have their own recorded acceptance gates.
Assert-Fails { & $bootstrap -DownloadOnly } 'Windows Server'
function Get-ItemProperty {
    param([string]$Path)
    if ($Path -ne 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion') { throw "Unexpected registry read: $Path" }
    [pscustomobject]@{CurrentBuildNumber = '26100'; InstallationType = 'Client'}
}
$savedArchitecture = $env:PROCESSOR_ARCHITEW6432
try {
    $env:PROCESSOR_ARCHITEW6432 = 'ARM64'
    Assert-Fails { & $bootstrap -DownloadOnly } 'ARM64'
} finally { $env:PROCESSOR_ARCHITEW6432 = $savedArchitecture }

# Corrupt downloads must be rejected before any process is launched and cleaned up.
$script:corruptPath = $null
function Invoke-WebRequest {
    param($Uri, $OutFile, $TimeoutSec, [switch]$UseBasicParsing)
    $script:corruptPath = $OutFile
    Set-Content -LiteralPath $OutFile -Value 'corrupt package'
}
Assert-Fails { & $bootstrap } 'checksum mismatch'
if (Test-Path (Split-Path $script:corruptPath)) { throw 'Failed download was not cleaned up.' }
Remove-Item Function:\Invoke-WebRequest

# Exercise the actual published package, checksum, install, update and cleanup.
# Add silent flags only in this test, without changing the public wizard flow.
$script:launchCount = 0
$script:lastInstaller = $null
function Start-Process {
    param($FilePath, $ArgumentList, [switch]$Wait, [switch]$PassThru)
    $script:launchCount++
    $script:lastInstaller = $FilePath
    Microsoft.PowerShell.Management\Start-Process -FilePath $FilePath -ArgumentList ($ArgumentList + @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/SP-')) -Wait -PassThru
}
try {
    & $bootstrap
    if (-not (Test-Path (Join-Path $app 'portrelay-desktop.exe'))) { throw 'Desktop launcher was not installed.' }
    if (Test-Path (Split-Path $script:lastInstaller)) { throw 'Installer download was not cleaned up.' }
    # The public one-liner evaluates the downloaded text, not a saved script file.
    Get-Content -LiteralPath $bootstrap -Raw | Invoke-Expression
    if ($script:launchCount -ne 2) { throw 'Install/update did not invoke the installer twice.' }
    if (Test-Path (Split-Path $script:lastInstaller)) { throw 'Update download was not cleaned up.' }
} finally {
    Remove-Item Function:\Start-Process
    $uninstaller = Join-Path $app 'unins000.exe'
    if (Test-Path $uninstaller) {
        $result = Start-Process $uninstaller -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART' -Wait -PassThru
        if ($result.ExitCode -ne 0) { throw "Uninstall failed: $($result.ExitCode)" }
    }
    Remove-Item Function:\Get-ItemProperty
}
Write-Host 'PASS: Server/ARM guards, corrupt download rejection, real package install/update, and temporary-file cleanup.'
