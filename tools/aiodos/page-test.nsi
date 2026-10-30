; Essai de la page « AIODOS » de l'installeur (build/installer.nsh), seule, sans rien installer :
;   makensis -INPUTCHARSET UTF8 -DLANG_ID=1036 -DROOT=<dossier d'ARKLIGHT> -DOUT=<page-fr.exe> tools\aiodos\page-test.nsi
; (makensis : celui d'electron-builder, %LOCALAPPDATA%\electron-builder\Cache\nsis-3.0.4.1\…\makensis.exe ;
; build\aiodos\AIODOS-Setup.exe doit exister : node tools/aiodos/prepare-installer.mjs)
Unicode true
!define BUILD_RESOURCES_DIR "${ROOT}\build"
!define AIODOS_PAGE_ALWAYS
!define isUpdated `1 == 0`

!include MUI2.nsh
Name "ARKLIGHT"
OutFile "${OUT}"
RequestExecutionLevel user
ShowInstDetails show

!include "${ROOT}\build\installer.nsh"
!insertmacro customPageAfterChangeDir
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "French"

Function .onInit
  StrCpy $LANGUAGE ${LANG_ID}
FunctionEnd

; rien n'est installé : on montre seulement ce que ferait la case cochée
Section
  ${If} $aiodosWanted == "1"
    DetailPrint "$(aiodosInstalling)"
  ${EndIf}
SectionEnd
