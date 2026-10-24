#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef SourceExe
  #define SourceExe "..\target\x86_64-pc-windows-msvc\release\recycle-desk.exe"
#endif
[Setup]
AppId={{DA427122-1CB5-4EE5-AC8D-58730802222C}
AppName=Recycle Desk
AppVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\Recycle Desk
DefaultGroupName=Recycle Desk
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.19045
OutputDir=..\dist
OutputBaseFilename=RecycleDesk-{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\recycle-desk.exe
CloseApplications=yes
RestartApplications=no
LicenseFile=..\LICENSE
[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "ukrainian"; MessagesFile: "compiler:Languages\Ukrainian.isl"
[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\THIRD_PARTY_NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion
[Icons]
Name: "{group}\Recycle Desk"; Filename: "{app}\recycle-desk.exe"
[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "RecycleDesk"; Flags: uninsdeletevalue
[Run]
Filename: "{app}\recycle-desk.exe"; Description: "{cm:LaunchProgram,Recycle Desk}"; Flags: nowait postinstall skipifsilent
