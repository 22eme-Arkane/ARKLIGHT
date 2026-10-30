; Installeur Windows de la version Rust d'ARKLIGHT (Inno Setup 6), dans le style sombre de celui
; d'AIODOS, aux couleurs d'ARKLIGHT (approuvé par l'auteur le 27/10/2026) : l'accueil (logo et
; présentation), la charte et les conditions d'utilisation à accepter, puis l'installation.
; Construit par tools/rust/build-installer.mjs, qui passe :
;   /DAppVersion=…  /DExe=… (arklight.exe)  /DRoot=… (dossier d'ARKLIGHT)  /DOutDir=…
;   et ce que prépare tools/rust/installer_pages.py : /DLegalFr=… /DLegalEn=… (RTF),
;   /DWizardImages=… /DWizardSmallImages=… (le logo, de 100 à 250 %), /DIcon=… (.ico)
;   /DAiodosSetup=… si l'installeur d'AIODOS est prêt (build\aiodos\AIODOS-Setup.exe).
; Nommée « ARKLIGHT » partout où l'utilisateur la voit (raccourcis, programmes installés). Pendant
; la migration, ses dossiers restent à part de ceux de la version JavaScript (dont les clés d'API
; et les réglages ont un autre format) : %LOCALAPPDATA%\Programs\ARKLIGHT-Rust, pour l'utilisateur seul, sans
; droits administrateur. Les projets (Documents\ARKLIGHT) et les réglages (%APPDATA%\ARKLIGHT-Rust)
; ne sont pas dans ce dossier : ils survivent à une nouvelle installation.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef OutDir
  #define OutDir "."
#endif
#ifndef LegalFr
  #error "LegalFr n'est pas défini : construire avec tools/rust/build-installer.mjs"
#endif

[Setup]
; Identifiant fixe : une nouvelle version remplace la précédente au lieu de s'y ajouter
AppId={{5C3E9A71-2B84-4F6D-A0C9-7E1D3B8F4A26}
AppName=ARKLIGHT
AppVersion={#AppVersion}
AppVerName=ARKLIGHT {#AppVersion}
AppPublisher=22eme Arkane
AppPublisherURL=https://github.com/22eme-Arkane/ARKLIGHT
AppSupportURL=https://github.com/22eme-Arkane/ARKLIGHT/issues
AppComments={cm:AppComments}
DefaultDirName={autopf}\ARKLIGHT-Rust
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir={#OutDir}
OutputBaseFilename=ARKLIGHT-Setup-{#AppVersion}
SetupIconFile={#Icon}
UninstallDisplayIcon={app}\arklight.exe
UninstallDisplayName=ARKLIGHT
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern dark
DisableWelcomePage=no
WizardImageFile={#WizardImages}
WizardSmallImageFile={#WizardSmallImages}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
CloseApplications=yes
RestartApplications=no
ShowLanguageDialog=no

[Languages]
; La langue de Windows choisit ; ni française ni anglaise : l'anglais. La charte et les
; conditions d'utilisation (rust/installer/legal), à accepter : elles citent la licence
; Apache-2.0, installée avec ARKLIGHT (LICENSE.txt, NOTICE.txt)
Name: "en"; MessagesFile: "compiler:Default.isl"; LicenseFile: "{#LegalEn}"
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"; LicenseFile: "{#LegalFr}"

[Messages]
; L'accueil : le texte de présentation, à côté du logo
fr.WelcomeLabel1=Bienvenue dans ARKLIGHT
fr.WelcomeLabel2=ARKLIGHT est un simulateur d'éclairage pour s'entraîner sans matériel. Place des projecteurs dans une scène en 3D, règle-les avec leurs vrais menus embarqués, pilote-les depuis une console DMX (Art-Net, sACN), une appli comme Blackout ou Luminair, ou un contrôleur MIDI, et vois la lumière tomber sur le décor.%n%nPrépare aussi le plan de feu, la liste du matériel et le patch, et demande de l'aide à l'assistant IA, à la voix avec ORAKLE. Tout tourne sur ton ordinateur, sans compte ni abonnement.%n%nCet assistant va installer [name/ver] sur ton ordinateur.
en.WelcomeLabel1=Welcome to ARKLIGHT
en.WelcomeLabel2=ARKLIGHT is a lighting simulator for practising without any gear. Place fixtures in a 3D scene, set them with their real on-board menus, control them from a DMX console (Art-Net, sACN), an app such as Blackout or Luminair, or a MIDI controller, and watch the light fall on the set.%n%nAlso prepare the lighting plot, the equipment list and the patch, and ask the AI assistant for help, by voice with ORAKLE. Everything runs on your computer, with no account and no subscription.%n%nThis wizard will install [name/ver] on your computer.
; La page de licence : la charte et les conditions d'utilisation
fr.WizardLicense=Charte et conditions d'utilisation
fr.LicenseLabel=Lis-les avant de continuer.
fr.LicenseLabel3=La charte dit l'esprit dans lequel on utilise ARKLIGHT ; les conditions d'utilisation, ses règles. Il faut les accepter pour installer ARKLIGHT.
fr.LicenseAccepted=J'&accepte la charte et les conditions d'utilisation
fr.LicenseNotAccepted=Je &refuse
en.WizardLicense=Charter and terms of use
en.LicenseLabel=Please read them before continuing.
en.LicenseLabel3=The charter sets out the spirit in which we use ARKLIGHT; the terms of use, its rules. You must accept them to install ARKLIGHT.
en.LicenseAccepted=I &accept the charter and the terms of use
en.LicenseNotAccepted=I &do not accept

[CustomMessages]
fr.AppComment=ARKLIGHT, le simulateur d'éclairage
en.AppComment=ARKLIGHT, the lighting simulator
fr.AppComments=Simulateur d'éclairage pour s'entraîner sans matériel
en.AppComments=Lighting simulator for practising without any gear
fr.AiodosTask=Installer aussi AIODOS, qui relie par Internet les ordinateurs de personnes qui ne se connaissent pas pour partager leur puissance de calcul. Il prête par défaut une part de ce PC au réseau public (60 % au plus, jamais sur batterie ; tout se coupe dans ses Réglages). Les demandes de l'assistant d'ARKLIGHT, elles, restent sur ce PC. Facultatif : son installeur s'ouvre ensuite.
en.AiodosTask=Also install AIODOS, which links the computers of people who don't know each other over the Internet to share their computing power. By default it lends part of this PC to the public network (60% at most, never on battery; it can all be turned off in its Settings). The ARKLIGHT assistant's requests, however, stay on this PC. Optional: its installer opens next.
fr.AiodosGroup=Compléments :
en.AiodosGroup=Add-ons:
fr.FinishedLabel=ARKLIGHT est installé. Il s'ouvre si la case ci-dessous reste cochée. Pour parler à l'assistant ou dicter dans les champs de texte, installe aussi ORAKLE, l'application de dictée d'ARKANE.
en.FinishedLabel=ARKLIGHT is installed. It opens if the box below stays checked. To talk to the assistant or dictate into text fields, also install ORAKLE, ARKANE's dictation app.
fr.DeleteSettings=Supprimer aussi tes réglages ARKLIGHT (langue, raccourcis, MIDI, clés de l'assistant) ? Tes projets, dans Documents\ARKLIGHT, ne sont jamais supprimés.
en.DeleteSettings=Also delete your ARKLIGHT settings (language, shortcuts, MIDI, assistant keys)? Your projects, in Documents\ARKLIGHT, are never deleted.

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
#ifdef AiodosSetup
Name: "aiodos"; Description: "{cm:AiodosTask}"; GroupDescription: "{cm:AiodosGroup}"; Flags: unchecked; Check: not AiodosInstalled
#endif

[InstallDelete]
; Restes d'une préversion installée avec NSIS (même dossier)
Type: files; Name: "{app}\Uninstall.exe"
; Raccourcis des préversions, nommés « ARKLIGHT (Rust) »
Type: files; Name: "{userprograms}\ARKLIGHT (Rust).lnk"
Type: files; Name: "{userdesktop}\ARKLIGHT (Rust).lnk"

[Files]
Source: "{#Exe}"; DestDir: "{app}"; DestName: "arklight.exe"; Flags: ignoreversion
Source: "{#Root}\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "{#Root}\NOTICE"; DestDir: "{app}"; DestName: "NOTICE.txt"; Flags: ignoreversion
Source: "{#Icon}"; DestDir: "{app}"; DestName: "arklight.ico"; Flags: ignoreversion
#ifdef AiodosSetup
; gardé dans le dossier d'ARKLIGHT : « Installer AIODOS » de l'assistant le lance plus tard
Source: "{#AiodosSetup}"; DestDir: "{app}\aiodos"; DestName: "AIODOS-Setup.exe"; Flags: ignoreversion
#endif

[Icons]
Name: "{userprograms}\ARKLIGHT"; Filename: "{app}\arklight.exe"; WorkingDir: "{app}"; Comment: "{cm:AppComment}"
Name: "{userdesktop}\ARKLIGHT"; Filename: "{app}\arklight.exe"; WorkingDir: "{app}"; Comment: "{cm:AppComment}"; Tasks: desktopicon

[Run]
#ifdef AiodosSetup
; L'installeur d'AIODOS, dans la langue de celui-ci : il montre sa propre charte
Filename: "{app}\aiodos\AIODOS-Setup.exe"; Parameters: "/LANG={language}"; Flags: waituntilterminated; Tasks: aiodos
#endif
Filename: "{app}\arklight.exe"; Description: "{cm:LaunchProgram,ARKLIGHT}"; Flags: nowait postinstall skipifsilent

[Code]
const
  // Préversion installée avec NSIS (rust/installer/arklight.nsi, retiré) : sa clé de désinstallation
  NsisKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\ARKLIGHT-Rust';

function AiodosInstalled: Boolean;
var
  Dir: String;
begin
  Result := RegQueryStringValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{8E1BB2BE-7BA8-4AF2-ACC7-463BA169FCF3}_is1', 'InstallLocation', Dir)
    or RegQueryStringValue(HKLM, 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{8E1BB2BE-7BA8-4AF2-ACC7-463BA169FCF3}_is1', 'InstallLocation', Dir)
    or FileExists(ExpandConstant('{localappdata}\Programs\AIODOS\runtime\pythonw.exe'));
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  if CurPageID = wpFinished then
    WizardForm.FinishedLabel.Caption := CustomMessage('FinishedLabel');
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  // l'entrée « ARKLIGHT (Rust) » de la préversion NSIS ne doit pas rester en double
  if CurStep = ssPostInstall then
    RegDeleteKeyIncludingSubkeys(HKCU, NsisKey);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Data: String;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    Data := ExpandConstant('{userappdata}\ARKLIGHT-Rust');
    if DirExists(Data) and not UninstallSilent and
       (MsgBox(CustomMessage('DeleteSettings') + #13#10#13#10 + Data, mbConfirmation,
               MB_YESNO or MB_DEFBUTTON2) = IDYES) then
      DelTree(Data, True, True, True);
  end;
end;
