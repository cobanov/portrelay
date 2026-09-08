param([ValidateSet('Stop','Start','Remove')][string]$Action)
$ErrorActionPreference='Stop'
$registry='HKLM:\SOFTWARE\PortRelay'
$install=$PSScriptRoot
$service=Get-Service PortRelayHelper -ErrorAction SilentlyContinue
if($service) {
    $config=Get-ItemProperty $registry
    if($config.InstallPath -ne $install){throw 'Another PortRelay installation owns the USB service.'}
    $actual=(Get-CimInstance Win32_Service -Filter "Name='PortRelayHelper'").PathName
    if($actual -ne ('"'+(Join-Path $install 'device-service\usbipd.exe')+'" server')){throw 'USB service configuration has changed. Restore it before updating PortRelay.'}
}
if($Action -eq 'Start') {
    if($service){Start-Service PortRelayHelper}
    exit 0
}
# Limit process termination to this installation's exact executable path.
foreach($process in Get-CimInstance Win32_Process -Filter "Name='portrelay.exe' OR Name='portrelay-desktop.exe'") {
    if($process.ExecutablePath -in @((Join-Path $install 'portrelay.exe'),(Join-Path $install 'portrelay-desktop.exe'))){Stop-Process -Id $process.ProcessId -Force -ErrorAction SilentlyContinue}
}
if($service -and $service.Status -ne 'Stopped') {
    Stop-Service PortRelayHelper
    (Get-Service PortRelayHelper).WaitForStatus('Stopped',[TimeSpan]::FromSeconds(45))
}
$recovery=Join-Path $env:ProgramData 'PortRelay\recovery'
if((Test-Path $recovery) -and (Get-ChildItem $recovery -Filter '*.json')) {
    if($service){Start-Service PortRelayHelper}
    throw 'USB devices still need recovery. Open PortRelay, disconnect the devices, and retry after recovery finishes.'
}
if($Action -eq 'Stop'){exit 0}
if(Test-Path $registry) {
    $config=Get-ItemProperty $registry
    $backend=Join-Path $install 'device-service\usbipd.exe'
    if($config.InstallPath -ne $install){throw 'Another PortRelay installation owns this configuration.'}
    if(Test-Path 'HKLM:\SOFTWARE\usbipd-win') {
        $state=(& $backend state | ConvertFrom-Json)
        if($LASTEXITCODE -ne 0){throw 'Could not verify USB device cleanup.'}
        if($state.Devices | Where-Object { $_.PersistedGuid -or $_.ClientIPAddress -or $_.IsForced }) {
            throw 'A USB device is still bound outside PortRelay. Release it before uninstalling.'
        }
    }
    $client=Join-Path $env:ProgramFiles 'USBip\usbip.exe'
    $ownsClient=$config.PSObject.Properties['InstalledClient'] -and $config.InstalledClient -eq 1
    if($ownsClient -and (Test-Path $client)) {
        $ports=& $client port
        if($LASTEXITCODE -ne 0){throw 'Cannot verify the USB controller. Restart Windows and retry uninstalling.'}
        if($ports -match '^Port\s+\d+:'){throw 'A USB device is still attached outside PortRelay. Disconnect it before uninstalling.'}
    }
    # Keep progress markers until every removal succeeds, so a retry after a
    # reboot does not attempt to uninstall an already removed driver package.
    $monitor=Get-Service VBoxUSBMon -ErrorAction SilentlyContinue
    if($monitor -and $monitor.Status -ne 'Stopped'){Stop-Service VBoxUSBMon}
    if(-not $config.PSObject.Properties['ExportDriverRemoved'] -or $config.ExportDriverRemoved -ne 1) {
        & $backend installer uninstall_driver
        if($LASTEXITCODE -ne 0){throw 'Windows could not remove the USB export driver. Restart and retry.'}
        New-ItemProperty -Path $registry -Name ExportDriverRemoved -Value 1 -PropertyType DWord -Force | Out-Null
    }
    if($ownsClient -and (Test-Path $client)) {
        $uninstaller=Join-Path $env:ProgramFiles 'USBip\unins000.exe'
        if(-not (Test-Path $uninstaller)){throw 'The USB client uninstaller is missing. Reinstall PortRelay USB support and retry.'}
        if(Get-CimInstance Win32_Process -Filter "Name='unins000.exe'" | Where-Object {$_.ExecutablePath -eq $uninstaller}) {
            throw 'Windows is still removing the USB controller. Restart Windows before retrying uninstall.'
        }
        $p=Start-Process $uninstaller -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART' -PassThru
        if(-not $p.WaitForExit(120000)) {
            # Do not terminate an in-progress kernel PnP operation or discard
            # ownership markers. A restart can finish it before the next retry.
            throw 'Windows has not released the USB controller. Restart Windows, then retry uninstalling PortRelay.'
        }
        $p.Refresh()
        if($p.ExitCode -notin @(0,3010)){throw 'The USB client could not be removed. Restart and retry.'}
    }
    if($service){& "$env:SystemRoot\System32\sc.exe" delete PortRelayHelper | Out-Null;if($LASTEXITCODE -ne 0){throw 'Could not remove the USB service.'}}
    if(Get-Service VBoxUSBMon -ErrorAction SilentlyContinue){& "$env:SystemRoot\System32\sc.exe" delete VBoxUSBMon | Out-Null;if($LASTEXITCODE -ne 0){throw 'Could not remove the USB monitor.'}}
    if(Test-Path 'HKLM:\SOFTWARE\usbipd-win'){Remove-Item 'HKLM:\SOFTWARE\usbipd-win' -Recurse}
    $run="Registry::HKEY_USERS\$($config.OwnerSid)\Software\Microsoft\Windows\CurrentVersion\Run"
    if(Test-Path $run){Remove-ItemProperty $run -Name PortRelay -ErrorAction SilentlyContinue}
    Remove-Item $registry -Recurse
}
Remove-NetFirewallRule -Name 'PortRelay.Encrypted' -ErrorAction SilentlyContinue
# User names, trusted computers, and settings stay in the owner's private profile.
