; ARKLIGHT — installeur Windows (electron-builder, NSIS) : page facultative « AIODOS ».
;
; AIODOS (d'ARKANE : la puissance de calcul partagée entre inconnus, par Internet ; les demandes
; d'ARKLIGHT, elles, restent sur ce PC) est proposé, jamais imposé :
; - la case est décochée par défaut ;
; - la page ne s'affiche ni si AIODOS est déjà installé, ni lors d'une mise à jour d'ARKLIGHT, ni
;   en installation silencieuse ;
; - cochée, l'installeur d'AIODOS (livré dans celui-ci) s'ouvre après la copie des fichiers
;   d'ARKLIGHT : c'est lui qui montre sa licence et ses options (dont le téléchargement d'Ollama).
; L'installeur d'AIODOS est préparé par tools/aiodos/prepare-installer.mjs (npm run dist) dans
; build/aiodos/ ; s'il n'y est pas, l'installeur d'ARKLIGHT est construit sans cette page.

!ifndef BUILD_UNINSTALLER
!if /FileExists "${BUILD_RESOURCES_DIR}\aiodos\AIODOS-Setup.exe"

!include nsDialogs.nsh
!include LogicLib.nsh

!define AIODOS_SETUP "${BUILD_RESOURCES_DIR}\aiodos\AIODOS-Setup.exe"
; clé de désinstallation d'AIODOS (Inno Setup, AppId fixe ; installation pour l'utilisateur)
!define AIODOS_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\{8E1BB2BE-7BA8-4AF2-ACC7-463BA169FCF3}_is1"

Var aiodosWanted
Var aiodosBox

; 1036 : français, 1033 : anglais (l'installeur prend la langue de Windows)
LangString aiodosTitle 1036 "AIODOS : la puissance partagée (facultatif)"
LangString aiodosTitle 1033 "AIODOS: shared computing power (optional)"
LangString aiodosSubtitle 1036 "Un complément gratuit d'ARKANE pour l'Assistant IA d'ARKLIGHT"
LangString aiodosSubtitle 1033 "A free ARKANE companion for the ARKLIGHT AI Assistant"
LangString aiodosText 1036 "AIODOS relie par Internet les ordinateurs de personnes qui ne se connaissent pas, pour partager leur puissance de calcul. Une fois installé, il prête par défaut une part de ce PC au réseau public : 60 % de sa puissance au plus, jamais sur batterie ; tout se coupe dans ses Réglages.$\r$\n$\r$\nLes demandes d'ARKLIGHT, elles, restent sur ce PC : AIODOS y devient le moteur de l'Assistant IA, qui agit sur le plan de feu et redessine un plan fait à la main d'après sa photo.$\r$\n$\r$\nARKLIGHT ne s'y connecte que si tu le décides (Assistant IA › « Se connecter à AIODOS », code à 6 chiffres). Sinon : Ollama ou une clé d'API."
LangString aiodosText 1033 "AIODOS links the computers of people who don't know each other over the Internet, so they can share their computing power. Once installed, it lends part of this PC to the public network by default: 60% of its power at most, never on battery; it can all be turned off in its Settings.$\r$\n$\r$\nARKLIGHT's requests, however, stay on this PC: AIODOS becomes the engine of the AI Assistant, which acts on the lighting plot and redraws a hand-drawn plot from its photo.$\r$\n$\r$\nARKLIGHT only connects to it when you decide to (AI Assistant › “Connect to AIODOS”, 6-digit code). Otherwise: Ollama or an API key."
LangString aiodosCheck 1036 "Installer aussi AIODOS"
LangString aiodosCheck 1033 "Also install AIODOS"
LangString aiodosNote 1036 "Son installeur s'ouvrira après celui d'ARKLIGHT (75 Mo, sans droits administrateur). Les modèles d'IA se téléchargent ensuite dans AIODOS, quand tu le choisis."
LangString aiodosNote 1033 "Its installer opens after ARKLIGHT's (75 MB, no administrator rights). AI models are then downloaded in AIODOS, when you choose."
LangString aiodosInstalling 1036 "Installation d'AIODOS (sa fenêtre est ouverte)…"
LangString aiodosInstalling 1033 "Installing AIODOS (its window is open)…"
LangString aiodosSkipped 1036 "AIODOS n'a pas été installé : tu pourras le faire plus tard."
LangString aiodosSkipped 1033 "AIODOS was not installed: you can do it later."

; Page après le choix du dossier (les fonctions sont définies ici : ${isUpdated} n'existe qu'à ce stade)
!macro customPageAfterChangeDir
  Page custom aiodosPageShow aiodosPageLeave

  ; $0 = "1" si AIODOS est déjà installé (pour cet utilisateur ou pour tous)
  Function aiodosFind
    StrCpy $0 "0"
    ClearErrors
    ReadRegStr $1 HKCU "${AIODOS_KEY}" "InstallLocation"
    ${If} $1 != ""
      StrCpy $0 "1"
      Return
    ${EndIf}
    ReadRegStr $1 HKLM "${AIODOS_KEY}" "InstallLocation"
    ${If} $1 != ""
      StrCpy $0 "1"
      Return
    ${EndIf}
    ${If} ${FileExists} "$LOCALAPPDATA\Programs\AIODOS\runtime\pythonw.exe"
      StrCpy $0 "1"
    ${EndIf}
  FunctionEnd

  Function aiodosPageShow
    ${If} ${isUpdated}
      Abort
    ${EndIf}
    !ifndef AIODOS_PAGE_ALWAYS ; essai de la page (tools/aiodos/page-test.nsi) : même si AIODOS est installé
      Call aiodosFind
      ${If} $0 == "1"
        Abort
      ${EndIf}
    !endif
    !insertmacro MUI_HEADER_TEXT "$(aiodosTitle)" "$(aiodosSubtitle)"
    nsDialogs::Create 1018
    Pop $0
    ${If} $0 == error
      Abort
    ${EndIf}
    ; la page fait environ 300 × 140 unités
    ${NSD_CreateLabel} 0 0 100% 100u "$(aiodosText)"
    Pop $0
    ${NSD_CreateCheckbox} 0 104u 100% 12u "$(aiodosCheck)"
    Pop $aiodosBox
    ${If} $aiodosWanted == "1"
      ${NSD_Check} $aiodosBox
    ${EndIf}
    ${NSD_CreateLabel} 12u 118u -12u 22u "$(aiodosNote)"
    Pop $0
    nsDialogs::Show
  FunctionEnd

  Function aiodosPageLeave
    ${NSD_GetState} $aiodosBox $0
    ${If} $0 == ${BST_CHECKED}
      StrCpy $aiodosWanted "1"
    ${Else}
      StrCpy $aiodosWanted "0"
    ${EndIf}
  FunctionEnd
!macroend

; Après la copie des fichiers d'ARKLIGHT : l'installeur d'AIODOS, dans la langue de celui-ci
!macro customInstall
  ${If} $aiodosWanted == "1"
    Push $0
    Push $1
    DetailPrint "$(aiodosInstalling)"
    File "/oname=$PLUGINSDIR\AIODOS-Setup.exe" "${AIODOS_SETUP}"
    StrCpy $1 "en"
    ${If} $LANGUAGE == 1036
      StrCpy $1 "fr"
    ${EndIf}
    ExecWait '"$PLUGINSDIR\AIODOS-Setup.exe" /LANG=$1' $0
    ${If} $0 != 0
      DetailPrint "$(aiodosSkipped)"
    ${EndIf}
    Delete "$PLUGINSDIR\AIODOS-Setup.exe"
    Pop $1
    Pop $0
  ${EndIf}
!macroend

!endif
!endif
