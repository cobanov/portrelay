param([switch]$SkipAgent)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$repo = Split-Path $PSScriptRoot -Parent
$target = Join-Path $repo 'target\windows-package'
$stage = Join-Path $target 'app'
$source = Join-Path $target 'usbipd-win'
$packages = Join-Path $repo 'target\packages'
New-Item -ItemType Directory -Force $stage,$packages | Out-Null
if (-not (Test-Path $source)) {
    git clone --branch v5.3.0 --single-branch https://github.com/dorssel/usbipd-win $source
    git -C $source switch -c main
    python (Join-Path $PSScriptRoot 'backend\prepare.py') $source
}
$revision = (git -C $source rev-parse HEAD).Trim()
if ($revision -ne 'aa3db8b82c4cb5071fd31bc54211606c70886912') { throw 'Unexpected USB backend source revision' }
Push-Location $source
try {
    dotnet tool restore
    dotnet publish Usbipd/Usbipd.csproj -c Release -r win-x64 -p:Platform=x64 -p:RuntimeIdentifiers=win-x64 -p:PublishSingleFile=true -p:PublishAot=false -p:PublishTrimmed=false -p:TreatWarningsAsErrors=false -o (Join-Path $stage 'device-service')
} finally { Pop-Location }
Copy-Item "$source\Drivers\x64\*.license" (Join-Path $stage 'device-service\Drivers')
Copy-Item "$source\LICENSES" (Join-Path $stage 'device-service\LICENSES') -Recurse -Force
Invoke-WebRequest 'https://raw.githubusercontent.com/dotnet/runtime/v9.0.19/LICENSE.TXT' -OutFile (Join-Path $stage 'device-service\LICENSES\DOTNET-LICENSE.txt')
Invoke-WebRequest 'https://raw.githubusercontent.com/dotnet/runtime/v9.0.19/THIRD-PARTY-NOTICES.TXT' -OutFile (Join-Path $stage 'device-service\LICENSES\DOTNET-NOTICES.txt')
New-Item -ItemType Directory -Force (Join-Path $stage 'drivers') | Out-Null
$driver = Join-Path $stage 'drivers\USBip-0.9.8.0-x64.exe'
if (-not (Test-Path $driver)) { Invoke-WebRequest 'https://github.com/vadimgrn/usbip-win2/releases/download/v.0.9.8.0/USBip-0.9.8.0-x64.exe' -OutFile $driver }
if ((Get-FileHash $driver -Algorithm SHA256).Hash -ne '81F426741F7EE2ED991FEBE24A22DACA8400B6AE2F171054E3FB404897E15D39') { throw 'USB import driver integrity check failed' }
Push-Location $repo
try {
    if (-not $SkipAgent) { cargo +1.97.0 build --locked --release }
    Copy-Item target/release/portrelay.exe,target/release/portrelay-desktop.exe $stage
    Copy-Item windows/packaging/*.ps1,LICENSE $stage
    Copy-Item windows/THIRD-PARTY-NOTICES.md $stage
    python scripts/dependency-notices.py
    python scripts/sbom.py
    Copy-Item target/packages/THIRD_PARTY_NOTICES.txt,target/packages/SBOM.cdx.json,Cargo.lock $stage
    $version = ((Select-String '^version = "([^"]+)"' Cargo.toml).Matches.Groups[1].Value)
    $compiler = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
    & $compiler "/DAppVersion=$version" "/DStage=$stage" "/DOutput=$packages" windows/packaging/portrelay.iss
    # Complete modified usbipd source, build additions, and upstream notices are
    # distributed next to the installer. Keep source archives independently useful.
    $corresponding = Join-Path $target 'corresponding-source'
    New-Item -ItemType Directory -Force $corresponding | Out-Null
    git -C $source archive HEAD -o (Join-Path $target 'upstream.tar')
    tar -xf (Join-Path $target 'upstream.tar') -C $corresponding
    Copy-Item "$source\Usbipd\*.cs" "$corresponding\Usbipd" -Force
    Copy-Item "$source\Usbipd\Usbipd.csproj","$source\Usbipd\NativeMethods.txt" "$corresponding\Usbipd" -Force
    Copy-Item "$source\global.json" $corresponding -Force
    Copy-Item windows/backend/packages.lock.json "$corresponding\Usbipd" -Force
    Copy-Item windows "$corresponding\PortRelay-build" -Recurse -Force
    Compress-Archive -Path "$corresponding\*" -DestinationPath "$packages\portrelay-$version-windows-backend-source.zip" -Force
    Get-ChildItem $packages -File | Where-Object { $_.Name -like "portrelay-$version-windows-*" -and $_.Extension -ne '.sha256' } | ForEach-Object {
        $hash = (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        [IO.File]::WriteAllText($_.FullName + '.sha256', "$hash  $($_.Name)`n", [Text.UTF8Encoding]::new($false))
    }
} finally { Pop-Location }
