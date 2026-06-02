; installer/setup.iss
;
; Spec: deploy/01-windows-build.md 1.4.1 (installer main frame) + 1.1.3 (OS compatibility
; preflight) + 1.4.2 (user data dirs) + 1.4.3 (uninstall keep-vs-purge choice). Implemented
; field-for-field. Compiled with Inno Setup 6.2.2 (ISCC.exe).
;
; DOCUMENTED SEAM (per task hard constraints): ISCC.exe (the Inno Setup compiler) is a genuine
; external prerequisite (deploy/00 0.7 baseline: Inno Setup 6.2.2). It is installed in CI via
; `choco install innosetup --version=6.2.2`. This .iss is the REAL compiler input; the SignTool=
; directive references the OV/EV cert thumbprint that is itself the signing seam (see 1.3).
;
; Dual binary (D1): trim_release.ps1 has already copied stuchka-core.exe into the Release dir and
; asserted its presence, so the recursive [Files] entry ships both binaries. No Emoji anywhere.
;
; Note: SignTool / signed uninstaller directives are commented until the cert thumbprint is
; available (the signing seam). Uncomment SignTool + SignedUninstaller once the OV cert is loaded.

#define MyAppName "Stučka"
#define MyAppVersion "0.1.0-beta.1"
#define MyAppPublisher "Stučka Project"
#define MyAppExeName "stuchka.exe"
; Release dir relative to this .iss (installer/ -> ../frontend/build/...). SourceDir below anchors it.
#define ReleaseDir "..\frontend\build\windows\x64\runner\Release"

[Setup]
AppId={{4E5F8B2A-7C9D-4F1E-8A6B-3D5C9E2F1B7A}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\Stuchka
DefaultGroupName={#MyAppName}
OutputDir=output
OutputBaseFilename=Stuchka-Setup-{#MyAppVersion}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
DisableDirPage=no
DisableProgramGroupPage=yes
LicenseFile=LICENSE-GPLv3.txt
InfoBeforeFile=README-FIRST.txt
; --- SIGNING SEAM (uncomment once the OV/EV cert thumbprint is available, deploy/01 1.3) ---
; SignTool=signtool sign /sha1 $q{#SignCertThumbprint}$q /fd sha256 /tr http://timestamp.digicert.com /td sha256 $f
; SignedUninstaller=yes

[Languages]
Name: "chinesesimplified"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"

[Files]
Source: "{#ReleaseDir}\stuchka.exe"; DestDir: "{app}"; Flags: ignoreversion
; Rust core subprocess binary ships with the installer (D1) -- trim_release.ps1 copied it into
; the Release dir and asserted its presence, so the recursive entry below also carries it; this
; explicit line documents the D1 dual-binary contract.
Source: "{#ReleaseDir}\stuchka-core.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ReleaseDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs
; GPG trust anchors (deploy/02 2.2.3 + deploy/04 4.8): app-upgrade pubkey + KB-distribution pubkey.
Source: "redist\stuchka-pubkey.asc"; DestDir: "{commonappdata}\Stuchka\trust"; Flags: ignoreversion
Source: "redist\stuchka-kb-pubkey.asc"; DestDir: "{commonappdata}\Stuchka\trust"; Flags: ignoreversion
; WebView2 offline bootstrapper -- only present in the enterprise offline build (deploy/01 1.1.2).
Source: "redist\MicrosoftEdgeWebview2Setup.exe"; DestDir: "{tmp}"; Flags: deleteafterinstall external skipifsourcedoesntexist; Check: WebView2NotInstalled

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "创建桌面快捷方式"; GroupDescription: "附加图标"

[Run]
Filename: "{tmp}\MicrosoftEdgeWebview2Setup.exe"; Parameters: "/silent /install"; Check: WebView2NotInstalled; StatusMsg: "正在安装 WebView2 运行时..."; Flags: skipifdoesntexist
Filename: "{app}\{#MyAppExeName}"; Description: "立即启动 {#MyAppName}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{app}\{#MyAppExeName}"; Parameters: "--uninstall-cleanup"; Flags: runhidden

[Code]
{ OS compatibility preflight (deploy/01 1.1.3): block Windows < 10 1909 (Build 18363). }
function InitializeSetup(): Boolean;
var
  WinMajor, WinBuild: Cardinal;
begin
  WinMajor := WindowsVersion shr 24;
  WinBuild := WindowsVersion and $FFFF;
  if (WinMajor < 10) or ((WinMajor = 10) and (WinBuild < 18363)) then
  begin
    MsgBox('Stučka 仅支持 Windows 10 1909 及以上 / Windows 11。当前系统不满足最低要求，安装中止。',
           mbCriticalError, MB_OK);
    Result := False;
    Exit;
  end;
  Result := True;
end;

function WebView2NotInstalled(): Boolean;
var
  Version: string;
begin
  Result := not RegQueryStringValue(HKLM,
    'SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
    'pv', Version);
end;

{ Uninstall keep-vs-purge choice (deploy/01 1.4.3): never default to purge. }
var
  KeepUserData: Boolean;

procedure InitializeUninstallProgressForm();
begin
  KeepUserData := MsgBox(
    '是否保留您的案件数据 / 证据 / 审计日志？' + #13#10 + #13#10 +
    '【是】保留：删除应用程序，保留 %LocalAppData%\Stuchka\ 目录及全部数据。' + #13#10 +
    '       未来重装时数据自动恢复。' + #13#10 +
    '【否】清除：彻底删除所有数据，无法恢复（请确认已导出 .stuchka-backup）。',
    mbConfirmation, MB_YESNO or MB_DEFBUTTON1) = IDYES;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  UserDataDir: string;
begin
  if CurUninstallStep = usPostUninstall then begin
    if not KeepUserData then begin
      UserDataDir := ExpandConstant('{localappdata}') + '\Stuchka';
      DelTree(UserDataDir, True, True, True);
      DelTree(ExpandConstant('{userappdata}') + '\Stuchka', True, True, True);
      { Windows Credential Manager entries are purged by stuchka.exe --uninstall-cleanup (UninstallRun). }
    end;
  end;
end;
