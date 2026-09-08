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
[Code]
function ServiceCommand(const Action: String): Boolean;
var Code: Integer;
begin
  Result := True;
  if FileExists(ExpandConstant('{app}\service-control.ps1')) then
    Result := Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
      '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + ExpandConstant('{app}\service-control.ps1') + '" -Action ' + Action,
      ExpandConstant('{app}'), SW_HIDE, ewWaitUntilTerminated, Code) and (Code = 0);
end;
function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  if not ServiceCommand('Stop') then
    Result := 'PortRelay could not return a USB device to its owner. Open PortRelay, finish recovery, and retry the update.';
end;
procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    if not ServiceCommand('Start') then
      MsgBox('The app was updated. Restart Windows to finish starting USB support.', mbInformation, MB_OK);
end;
function InitializeUninstall(): Boolean;
begin
  Result := ServiceCommand('Remove');
  if not Result then
    MsgBox('PortRelay could not finish USB cleanup. Restart Windows, open PortRelay, disconnect devices, and retry uninstalling.', mbError, MB_OK);
end;
