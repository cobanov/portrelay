#ifndef AppVersion
  #error AppVersion is required
#endif
[Setup]
AppId={{EE9C1A49-C3D0-47DB-86C2-642593F45BC0}
AppName=PortRelay
AppVersion={#AppVersion}
AppPublisher=Muhammed Cobanov
AppPublisherURL=https://portrelay.cobanov.dev
AppSupportURL=https://github.com/cobanov/portrelay/issues
DefaultDirName={autopf}\PortRelay
DisableDirPage=yes
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.22000
PrivilegesRequired=admin
OutputDir={#Output}
OutputBaseFilename=portrelay-{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
UninstallDisplayIcon={app}\portrelay-desktop.exe
LicenseFile={#Stage}\LICENSE
SetupLogging=yes
[Files]
Source: "{#Stage}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs restartreplace uninsrestartdelete
[Icons]
Name: "{commonprograms}\PortRelay"; Filename: "{app}\portrelay-desktop.exe"; WorkingDir: "{app}"
[Run]
Filename: "{app}\portrelay-desktop.exe"; Description: "Open PortRelay"; Flags: nowait postinstall skipifsilent runasoriginaluser; WorkingDir: "{app}"
