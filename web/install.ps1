# PortRelay release bootstrapper. MIT licensed; source and notices:
# https://github.com/cobanov/portrelay
# Usage: irm https://portrelay.cobanov.dev/install.ps1 | iex
[CmdletBinding()]
param([switch]$DownloadOnly)

& {
    $ErrorActionPreference = 'Stop'
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        throw 'This installer requires Windows 11 on Intel/AMD 64-bit hardware.'
    }
    $nativeArchitecture = $env:PROCESSOR_ARCHITEW6432
    if (-not $nativeArchitecture) { $nativeArchitecture = $env:PROCESSOR_ARCHITECTURE }
    if ($nativeArchitecture -ne 'AMD64') {
        throw 'This alpha requires Intel/AMD 64-bit Windows. ARM64 is not supported.'
    }
    $windows = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
    if ([int]$windows.CurrentBuildNumber -lt 22000 -or $windows.InstallationType -ne 'Client') {
        throw 'This alpha requires Windows 11. Windows 10 and Windows Server are not supported.'
    }

    $version = '0.1.0-alpha.6'
    $package = "portrelay-$version-windows-x64-setup.exe"
    $checksum = '6137a9e2e17e5d79b20d2f692387370076585017b6aa4a19fa451cbc46be8bbf'
    $tempDirectory = Join-Path ([IO.Path]::GetTempPath()) ('portrelay-' + [Guid]::NewGuid().ToString('N'))
    $installer = Join-Path $tempDirectory $package
    $previousProtocol = [Net.ServicePointManager]::SecurityProtocol
    try {
        $null = New-Item -ItemType Directory -Path $tempDirectory
        [Net.ServicePointManager]::SecurityProtocol = $previousProtocol -bor [Net.SecurityProtocolType]::Tls12
        Write-Host "Downloading PortRelay $version for Windows..."
        Invoke-WebRequest -UseBasicParsing -Uri "https://github.com/cobanov/portrelay/releases/download/v$version/$package" -OutFile $installer -TimeoutSec 600
        if ((Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash -ne $checksum) {
            throw 'Package checksum mismatch. Nothing was installed. Please retry or report this on GitHub.'
        }
        Write-Host 'Package checksum verified.'
        if ($DownloadOnly) { return }

        Write-Host 'Opening the alpha installer. Approve the Windows administrator prompt to continue.'
        # Let the existing installer request UAC, retain the original user, and show its wizard.
        # Never change execution policy, SmartScreen, Secure Boot, or signature enforcement.
        $process = Start-Process -FilePath $installer -ArgumentList '/NORESTART', '/RESTARTEXITCODE=3010' -Wait -PassThru
        if ($process.ExitCode -eq 3010) {
            Write-Host 'Installed. Restart Windows, then open PortRelay from Start.'
        } elseif ($process.ExitCode -eq 0) {
            Write-Host 'Installed! Open PortRelay, choose Enable USB sharing, then Add computer.'
        } else {
            throw "The installer did not finish (exit code $($process.ExitCode)). Retry or use the Windows setup guide."
        }
    } finally {
        [Net.ServicePointManager]::SecurityProtocol = $previousProtocol
        if (Test-Path -LiteralPath $tempDirectory) {
            Remove-Item -LiteralPath $tempDirectory -Recurse -Force
        }
    }
}
