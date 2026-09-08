param([Parameter(Mandatory=$true)][string]$OwnerSid)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$install = $PSScriptRoot
$backend = Join-Path $install 'device-service\usbipd.exe'
$root = Join-Path $env:ProgramData 'PortRelay'
$registry = 'HKLM:\SOFTWARE\PortRelay'
$usbRegistry = 'HKLM:\SOFTWARE\usbipd-win'
$mutex = New-Object Threading.Mutex($false, 'Global\PortRelay.Setup.v1')
if (-not $mutex.WaitOne(0)) { throw 'USB setup is already running.' }
function Assert-ProtectedPath([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return }
    if ((Get-Item -LiteralPath $Path -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'The USB service data must not contain links.' }
    $owner=(Get-Acl -LiteralPath $Path).GetOwner([Security.Principal.SecurityIdentifier]).Value
    if ($owner -notin @('S-1-5-18','S-1-5-32-544')) { throw 'The USB service data must belong to Windows administrators. Remove the conflicting PortRelay data directory and retry.' }
}
try {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not (New-Object Security.Principal.WindowsPrincipal($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Windows administrator permission is required.' }
    $sid = New-Object Security.Principal.SecurityIdentifier($OwnerSid)
    if ($sid.Value -notmatch '^S-1-5-21-\d+-\d+-\d+-\d+$') { throw 'Choose a normal Windows user to own USB sharing.' }
    $null = $sid.Translate([Security.Principal.NTAccount])
    if (Test-Path $registry) {
        $existing = Get-ItemProperty $registry
        if (($existing.PSObject.Properties['OwnerSid'] -and $existing.OwnerSid -ne $OwnerSid) -or ($existing.PSObject.Properties['InstallPath'] -and $existing.InstallPath -ne $install)) { throw 'Another Windows user or installation owns PortRelay USB support.' }
    } else {
        if ((Test-Path $usbRegistry) -or (Get-Service usbipd,VBoxUSBMon -ErrorAction SilentlyContinue)) {
            throw 'An existing usbipd-win or VirtualBox USB service is installed. PortRelay will not replace it. Remove that USB service first if you want PortRelay to manage these devices.'
        }
    }
    $client = Join-Path $env:ProgramFiles 'USBip\usbip.exe'
    if ((Test-Path $client) -and (-not (Test-Path $registry) -or (Get-ItemPropertyValue $registry InstalledClient -ErrorAction SilentlyContinue) -ne 1)) {
        throw 'An existing USBip installation is present. PortRelay needs its own USB controller. Uninstall USBip first if you want PortRelay to manage it.'
    }
    # Keep service configuration, logs, and recovery journals administrator-only.
    Assert-ProtectedPath $root
    $null = New-Item -ItemType Directory -Force $root
    $acl = New-Object Security.AccessControl.DirectorySecurity
    $acl.SetAccessRuleProtection($true, $false)
    $acl.SetOwner((New-Object Security.Principal.SecurityIdentifier('S-1-5-32-544')))
    foreach ($account in @('S-1-5-18', 'S-1-5-32-544')) {
        $rule = New-Object Security.AccessControl.FileSystemAccessRule((New-Object Security.Principal.SecurityIdentifier($account)), 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow')
        $acl.AddAccessRule($rule)
    }
    Set-Acl -LiteralPath $root -AclObject $acl
    Assert-ProtectedPath (Join-Path $root 'setup.log')
    Assert-ProtectedPath (Join-Path $root 'recovery')
    if (Test-Path (Join-Path $root 'recovery')) {
        foreach ($record in Get-ChildItem -LiteralPath (Join-Path $root 'recovery') -Force) {
            Assert-ProtectedPath $record.FullName
            if ($record.PSIsContainer) { throw 'Unexpected directory in USB recovery data.' }
        }
    }
    Start-Transcript -Path (Join-Path $root 'setup.log') -Append | Out-Null
    $null = New-Item -ItemType Directory -Force (Join-Path $root 'recovery')
    $null = New-Item -Force $registry
    New-ItemProperty -Path $registry -Name OwnerSid -Value $OwnerSid -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $registry -Name InstallPath -Value $install -PropertyType String -Force | Out-Null
    $clientInstaller = Join-Path $install 'drivers\USBip-0.9.8.0-x64.exe'
    $hash = (Get-FileHash -LiteralPath $clientInstaller -Algorithm SHA256).Hash
    if ($hash -ne '81F426741F7EE2ED991FEBE24A22DACA8400B6AE2F171054E3FB404897E15D39') { throw 'The USB driver download failed integrity verification. Reinstall PortRelay.' }
    if (-not (Test-Path $client)) {
        $process = Start-Process -FilePath $clientInstaller -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- /COMPONENTS=main,client' -Wait -PassThru
        if ($process.ExitCode -notin @(0, 3010)) { throw "The signed USB driver installer failed ($($process.ExitCode)). Restart Windows and retry." }
        New-ItemProperty -Path $registry -Name InstalledClient -Value 1 -PropertyType DWord -Force | Out-Null
    } elseif ((Get-Item $client).VersionInfo.FileVersion -notlike '0.9.8.0*') {
        throw 'A different USBip client version is installed. Update it to the signed 0.9.8.0 release before continuing.'
    }
    & (Join-Path $install 'protect-controller.ps1')
    $null = New-Item -Force $usbRegistry
    New-ItemProperty -Path $usbRegistry -Name APPLICATIONFOLDER -Value (Join-Path $install 'device-service') -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $usbRegistry -Name Version -Value '5.3.0' -PropertyType String -Force | Out-Null
    $null = New-Item -Force "$usbRegistry\Devices"
    $null = New-Item -Force "$usbRegistry\Policy"
    & $backend installer install_driver
    if ($LASTEXITCODE -ne 0) { throw 'Windows rejected the signed USB export driver.' }
    New-ItemProperty -Path $registry -Name ExportDriverRemoved -Value 0 -PropertyType DWord -Force | Out-Null
    if (-not (Get-Service VBoxUSBMon -ErrorAction SilentlyContinue)) {
        & $backend installer install_monitor
        if ($LASTEXITCODE -ne 0) { throw 'Windows could not install the USB monitor driver.' }
    }
    $service = Get-Service PortRelayHelper -ErrorAction SilentlyContinue
    if ($service) { Restart-Service PortRelayHelper }
    else {
        New-Service -Name PortRelayHelper -DisplayName 'PortRelay USB service' -BinaryPathName ('"' + $backend + '" server') -StartupType Automatic | Out-Null
        & "$env:SystemRoot\System32\sc.exe" failure PortRelayHelper reset= 86400 actions= restart/3000/restart/10000/restart/30000 | Out-Null
        Start-Service PortRelayHelper
    }
    if (-not (Get-NetFirewallRule -Name 'PortRelay.Encrypted' -ErrorAction SilentlyContinue)) {
        New-NetFirewallRule -Name 'PortRelay.Encrypted' -DisplayName 'PortRelay encrypted device connections' -Direction Inbound -Action Allow -Protocol UDP -Program (Join-Path $install 'portrelay.exe') -Profile Any | Out-Null
    } else { Set-NetFirewallRule -Name 'PortRelay.Encrypted' -Profile Any | Out-Null }
    Write-Output 'USB support is installed. Restart Windows if the app still asks you to enable USB support.'
} catch {
    Write-Error $_
    exit 1
} finally {
    try { Stop-Transcript | Out-Null } catch { }
    $mutex.ReleaseMutex()
    $mutex.Dispose()
}
