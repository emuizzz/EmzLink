; EmzLink installer script for Inno Setup 6.3+ (https://jrsoftware.org/isinfo.php)
; Put this file in the "installer" folder next to EmzLink.exe, open it in Inno Setup and press Compile (F9).
; The result: installer\Output\EmzLink-Setup.exe

#define AppName "EmzLink"
#define AppVersion "1.0.0"

[Setup]
AppId={{9D4E6C1A-7B3F-4E58-A2C6-5F1B0E8D3A47}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=EmzProject
AppPublisherURL=https://t.me/EmzProject
; zapret .bat files break on spaces and non-Latin letters, so NOT "Program Files"
DefaultDirName=C:\EmzLink
DisableProgramGroupPage=yes
DisableDirPage=no
PrivilegesRequired=admin
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=Output
OutputBaseFilename=EmzLink-Setup
SetupIconFile=..\source\src-tauri\icons\icon.ico
UninstallDisplayIcon={app}\EmzLink.exe
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=force
RestartApplications=no

[Languages]
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"

[Tasks]
Name: "desktopicon"; Description: "Ярлык на рабочем столе"; Flags: unchecked

[Files]
Source: "..\*"; DestDir: "{app}"; Excludes: "installer\*,source\*,.github\*,.gitignore,Reset-Settings.bat,*.zip"; Flags: recursesubdirs createallsubdirs ignoreversion

[Icons]
Name: "{autoprograms}\EmzLink"; Filename: "{app}\EmzLink.exe"
Name: "{autodesktop}\EmzLink"; Filename: "{app}\EmzLink.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\EmzLink.exe"; Description: "Запустить EmzLink"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "taskkill"; Parameters: "/F /T /IM EmzLink.exe"; Flags: runhidden; RunOnceId: "KillApp"
Filename: "taskkill"; Parameters: "/F /T /IM winws.exe"; Flags: runhidden; RunOnceId: "KillWinws"
Filename: "taskkill"; Parameters: "/F /T /IM TgWsProxy.exe"; Flags: runhidden; RunOnceId: "KillTg"
Filename: "schtasks"; Parameters: "/Delete /TN EmzLinkApp /F"; Flags: runhidden; RunOnceId: "TaskApp"
Filename: "schtasks"; Parameters: "/Delete /TN EmzLinkBypass /F"; Flags: runhidden; RunOnceId: "TaskBypass"
Filename: "schtasks"; Parameters: "/Delete /TN EmzLinkTgProxy /F"; Flags: runhidden; RunOnceId: "TaskTg"
Filename: "sc"; Parameters: "stop WinDivert"; Flags: runhidden; RunOnceId: "DrvStop"

[UninstallDelete]
Type: filesandordirs; Name: "{app}\tgproxy"
