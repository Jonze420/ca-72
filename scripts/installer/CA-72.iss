; The CA-72's installer for Windows (Inno Setup 6), made by scripts/package.sh from the
; bundles it stages: CA-72.vst3 into Common Files\VST3, CA-72.clap into Common Files\CLAP,
; and the licence, the notices and the uninstaller into Program Files\Idle Foundry\CA-72
; (decisions.md R17). Defines: Version, Stage (the staged files), Output (where it goes).

#ifndef Version
  #error Run from scripts/package.sh, which defines Version, Stage and Output
#endif

[Setup]
; Never change the AppId: it is how a later installer finds this one's installation.
AppId={{9FF1B860-34D5-4A52-8E33-91FCAF8CF5B2}
AppName=CA-72
AppVersion={#Version}
AppVerName=CA-72 {#Version}
AppPublisher=Idle Foundry Ltd.
AppPublisherURL=https://github.com/idlefoundry/ca-72
AppSupportURL=https://github.com/idlefoundry/ca-72/issues
AppUpdatesURL=https://github.com/idlefoundry/ca-72/releases
AppCopyright=Copyright (C) 2026 Idle Foundry Ltd. GNU GPL version 3 or later.
DefaultDirName={commonpf64}\Idle Foundry\CA-72
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableWelcomePage=no
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#Output}
OutputBaseFilename=CA-72-{#Version}-Windows-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
UninstallDisplayName=CA-72 {#Version}
VersionInfoVersion={#Version}
VersionInfoCompany=Idle Foundry Ltd.
VersionInfoDescription=CA-72 installer

[Types]
Name: "full"; Description: "VST3 and CLAP"
Name: "custom"; Description: "Choose the formats"; Flags: iscustom

[Components]
Name: "vst3"; Description: "VST3, in C:\Program Files\Common Files\VST3"; Types: full custom
Name: "clap"; Description: "CLAP, in C:\Program Files\Common Files\CLAP"; Types: full custom

[InstallDelete]
; A bundle from an earlier version goes first, so that none of its files outlive it.
Type: filesandordirs; Name: "{commoncf64}\VST3\CA-72.vst3"; Components: vst3

[Files]
Source: "{#Stage}\CA-72.vst3\*"; DestDir: "{commoncf64}\VST3\CA-72.vst3"; Components: vst3; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#Stage}\CA-72.clap"; DestDir: "{commoncf64}\CLAP"; Components: clap; Flags: ignoreversion
Source: "{#Stage}\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\THIRD-PARTY-NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion

[UninstallDelete]
Type: filesandordirs; Name: "{commoncf64}\VST3\CA-72.vst3"

[Code]
// The maker's folder goes with the last of its programs (RemoveDir leaves it if not empty).
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemoveDir(ExpandConstant('{commonpf64}\Idle Foundry'));
end;
