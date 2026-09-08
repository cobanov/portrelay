# Rebuild the accompanying corresponding-source archive without a .git directory.
$ErrorActionPreference='Stop'
$PSNativeCommandUseErrorActionPreference=$true
Push-Location $PSScriptRoot
try {
    if (-not (Test-Path 'Usbipd/PortRelayArchiveVersion.cs')) { throw 'Run this script from the extracted PortRelay Windows backend source archive.' }
    dotnet publish Usbipd/Usbipd.csproj -c Release -r win-x64 -p:Platform=x64 -p:RuntimeIdentifiers=win-x64 -p:PublishSingleFile=true -p:PublishAot=false -p:PublishTrimmed=false -p:RestoreLockedMode=true -p:DisableGitVersionTask=true -p:ContinuousIntegrationBuild=false -p:Version=5.3.0 -p:AssemblyVersion=5.3.0.0 -p:FileVersion=5.3.0.0 -p:TreatWarningsAsErrors=false -o rebuilt-service
} finally { Pop-Location }
