; Rekaptr Inno Setup installer
; Builds rekaptr-setup-<version>.exe from the dist-built binary + runtime/ tree.
; Writes an axoupdater-compatible install receipt so in-app updates work.

#define MyAppName "Rekaptr"
#define MyAppNameLower "rekaptr"
#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif
#define MyAppPublisher "Sierra8953"
#define MyAppURL "https://github.com/Sierra8953/Rekaptr"
#define MyAppExeName "rekaptr.exe"

[Setup]
AppId={{6F2A1C8E-9C5E-4B6E-A0E4-3D2C1B8F5A11}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}/issues
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
LicenseFile=LICENSE
OutputDir=target\installer
OutputBaseFilename=rekaptr-setup-{#MyAppVersion}
Compression=lzma2/max
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern
PrivilegesRequired=admin
CloseApplications=force
RestartApplications=no
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Files]
Source: "target\x86_64-pc-windows-msvc\dist\rekaptr.exe"; DestDir: "{app}"; Flags: ignoreversion
; runtime\ holds the bundled GStreamer/libmpv DLLs plus ffmpeg.exe AND ffprobe.exe,
; copied next to the exe where the app discovers them. ffprobe is required for
; cross-session recording (decode-time-offset); build-release.ps1 verifies it exists.
Source: "runtime\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent

[Code]
procedure WriteInstallReceipt();
var
  ReceiptDir: string;
  ReceiptPath: string;
  EscapedPrefix: string;
  Json: string;
begin
  ReceiptDir := ExpandConstant('{localappdata}\{#MyAppNameLower}');
  if not DirExists(ReceiptDir) then
    ForceDirectories(ReceiptDir);
  ReceiptPath := ReceiptDir + '\{#MyAppNameLower}-receipt.json';

  EscapedPrefix := ExpandConstant('{app}');
  StringChangeEx(EscapedPrefix, '\', '\\', True);

  Json :=
    '{"binaries":["{#MyAppExeName}"],' +
    '"binary_aliases":{},' +
    '"cdylibs":[],' +
    '"cstaticlibs":[],' +
    '"install_layout":"unspecified",' +
    '"install_prefix":"' + EscapedPrefix + '",' +
    '"modify_path":false,' +
    '"provider":{"source":"inno-setup","version":"6.0.0"},' +
    '"source":{"app_name":"{#MyAppNameLower}","name":"{#MyAppName}","owner":"Sierra8953","release_type":"github"},' +
    '"version":"{#MyAppVersion}"}';

  SaveStringToFile(ReceiptPath, Json, False);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    WriteInstallReceipt();
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  ReceiptDir: string;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    ReceiptDir := ExpandConstant('{localappdata}\{#MyAppNameLower}');
    DelTree(ReceiptDir, True, True, True);
  end;
end;
