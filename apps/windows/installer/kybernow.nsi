; Installateur de l'app KyberNow pour Windows, par utilisateur (sans droits
; d'administrateur), séparé de celui de l'ordinateur partagé (serveur).
; Compilé sur le Mac par apps/windows/scripts/build-installer.sh :
;   makensis -DVERSION=X.Y.Z -DARCH=x64|arm64 -DSRC=<dist/arch> -DOUT=<exe> kybernow.nsi
;
; /S : silencieux ; /UPDATE : mise à jour lancée par l'app (relance ensuite).
; Avant de copier, attend que l'app se ferme (mutex Local\KyberNow.App).

Unicode true
SetCompressor /SOLID lzma
RequestExecutionLevel user
ManifestDPIAware true

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"

!ifndef VERSION
  !error "VERSION manquant"
!endif

Name "KyberNow"
OutFile "${OUT}"
InstallDir "$LOCALAPPDATA\Programs\KyberNow"
BrandingText "KyberNow ${VERSION} · Les Lignes Claires"

VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1036 "ProductName" "KyberNow"
VIAddVersionKey /LANG=1036 "CompanyName" "Les Lignes Claires"
VIAddVersionKey /LANG=1036 "FileDescription" "Installation de KyberNow (${ARCH})"
VIAddVersionKey /LANG=1036 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1036 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1036 "LegalCopyright" "AGPL-3.0, Les Lignes Claires"

!define UNINST "Software\Microsoft\Windows\CurrentVersion\Uninstall\KyberNow"
!define WV2 "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

!define MUI_ICON "${RES}\kybernow.ico"
!define MUI_UNICON "${RES}\kybernow.ico"
!define MUI_WELCOMEFINISHPAGE_BITMAP "${RES}\welcome.bmp"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "${RES}\welcome.bmp"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_RIGHT
!define MUI_HEADERIMAGE_BITMAP "${RES}\header.bmp"
!define MUI_ABORTWARNING

!define MUI_WELCOMEPAGE_TITLE "KyberNow"
!define MUI_WELCOMEPAGE_TEXT "Votre ordinateur sur ce PC : l'image, le son, le clavier, la souris et les manettes, avec la latence la plus basse.$\r$\n$\r$\nKyberNow s'installe pour vous seul, sans droits d'administrateur, dans :$\r$\n$INSTDIR$\r$\n$\r$\nVersion ${VERSION} (${ARCH})."
!define MUI_FINISHPAGE_TITLE "KyberNow est installée"
!define MUI_FINISHPAGE_TEXT "Vous la retrouvez dans le menu Démarrer. Ajoutez votre ordinateur depuis l'accueil : sur le réseau, avec le lien de sa console ou par son adresse."
!define MUI_FINISHPAGE_RUN "$INSTDIR\kybernow.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Ouvrir KyberNow"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "French"

Var Update

Function .onInit
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/UPDATE" $R1
  ${IfNot} ${Errors}
    StrCpy $Update 1
  ${EndIf}
!if "${ARCH}" == "arm64"
  ; Version ARM64 : seulement sur un Windows ARM64.
  ReadEnvStr $0 PROCESSOR_ARCHITEW6432
  ReadEnvStr $1 PROCESSOR_ARCHITECTURE
  ${If} $0 != "ARM64"
  ${AndIf} $1 != "ARM64"
    MessageBox MB_ICONSTOP "Cette version de KyberNow est pour les PC ARM (Snapdragon). Prenez KyberNow-Windows-x64.exe." /SD IDOK
    Abort
  ${EndIf}
!endif
FunctionEnd

; Attend que l'app soit fermée (20 s), puis la ferme si elle ne l'est pas.
!macro WAIT_FOR_APP un
Function ${un}WaitForApp
  StrCpy $1 0
  loop:
    System::Call 'kernel32::OpenMutexW(i 0x00100000, i 0, w "Local\KyberNow.App") p .r0'
    ${If} $0 == 0
      Return
    ${EndIf}
    System::Call 'kernel32::CloseHandle(p r0)'
    ${If} $1 == 0
      DetailPrint "Attente de la fermeture de KyberNow…"
    ${EndIf}
    IntOp $1 $1 + 1
    ${If} $1 > 100
      ${IfNot} ${Silent}
        ${If} ${Cmd} `MessageBox MB_OKCANCEL|MB_ICONEXCLAMATION "KyberNow est ouverte. Fermez-la pour continuer (OK la fermera)." IDCANCEL`
          Abort
        ${EndIf}
      ${EndIf}
      nsExec::Exec 'taskkill /IM kybernow.exe /F'
      Pop $0
      Sleep 1000
      Return
    ${EndIf}
    Sleep 200
    Goto loop
FunctionEnd
!macroend
!insertmacro WAIT_FOR_APP ""
!insertmacro WAIT_FOR_APP "un."

; Moteur WebView2 d'Edge (l'interface de l'app) : présent sur Windows 11 et la
; plupart des Windows 10 ; sinon installé pour cet utilisateur par
; l'installateur officiel de Microsoft.
Function EnsureWebView2
  SetRegView 32
  ReadRegStr $0 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WV2}" "pv"
  SetRegView 64
  ${If} $0 == ""
  ${OrIf} $0 == "0.0.0.0"
    ReadRegStr $0 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WV2}" "pv"
  ${EndIf}
  ${If} $0 == ""
  ${OrIf} $0 == "0.0.0.0"
    ReadRegStr $0 HKCU "Software\Microsoft\EdgeUpdate\Clients\${WV2}" "pv"
  ${EndIf}
  ${If} $0 != ""
  ${AndIf} $0 != "0.0.0.0"
    DetailPrint "WebView2 $0 présent"
    Return
  ${EndIf}
  DetailPrint "Installation de WebView2 (Microsoft)…"
  ; Le chemin passe par une variable d'environnement : une apostrophe dans le
  ; nom du dossier temporaire ne casse pas la commande.
  System::Call 'kernel32::SetEnvironmentVariableW(w "KYBERNOW_WV2", w "$TEMP\MicrosoftEdgeWebview2Setup.exe")'
  nsExec::ExecToLog `powershell -NoProfile -ExecutionPolicy Bypass -Command "$$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile $$env:KYBERNOW_WV2"`
  Pop $1
  ${If} ${FileExists} "$TEMP\MicrosoftEdgeWebview2Setup.exe"
    ExecWait '"$TEMP\MicrosoftEdgeWebview2Setup.exe" /silent /install' $1
    Delete "$TEMP\MicrosoftEdgeWebview2Setup.exe"
  ${EndIf}
  ${If} $1 != 0
    MessageBox MB_ICONEXCLAMATION "WebView2 n'a pas pu être installé. KyberNow en a besoin pour son interface : installez « Microsoft Edge WebView2 Runtime » depuis le site de Microsoft." /SD IDOK
  ${EndIf}
FunctionEnd

Section "KyberNow"
  Call WaitForApp
  SetOutPath "$INSTDIR"
  ; Une mise à jour remplace tout le dossier de l'app (pas les données, dans
  ; %APPDATA% et %LOCALAPPDATA%).
  File /r "${SRC}\*.*"
  WriteUninstaller "$INSTDIR\Desinstaller.exe"
  CreateShortcut "$SMPROGRAMS\KyberNow.lnk" "$INSTDIR\kybernow.exe" "" "$INSTDIR\kybernow.exe" 0 SW_SHOWNORMAL "" "Votre ordinateur sur ce PC"

  WriteRegStr HKCU "${UNINST}" "DisplayName" "KyberNow"
  WriteRegStr HKCU "${UNINST}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST}" "Publisher" "Les Lignes Claires"
  WriteRegStr HKCU "${UNINST}" "URLInfoAbout" "https://www.kybernow.app"
  WriteRegStr HKCU "${UNINST}" "DisplayIcon" "$INSTDIR\kybernow.exe"
  WriteRegStr HKCU "${UNINST}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST}" "UninstallString" '"$INSTDIR\Desinstaller.exe"'
  WriteRegStr HKCU "${UNINST}" "QuietUninstallString" '"$INSTDIR\Desinstaller.exe" /S'
  WriteRegDWORD HKCU "${UNINST}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  WriteRegDWORD HKCU "${UNINST}" "EstimatedSize" $0

  ; Lien kybernow:// (l'app le refait à chaque lancement).
  WriteRegStr HKCU "Software\Classes\kybernow" "" "URL:KyberNow"
  WriteRegStr HKCU "Software\Classes\kybernow" "URL Protocol" ""
  WriteRegStr HKCU "Software\Classes\kybernow\DefaultIcon" "" '"$INSTDIR\kybernow.exe",0'
  WriteRegStr HKCU "Software\Classes\kybernow\shell\open\command" "" '"$INSTDIR\kybernow.exe" "%1"'

  Call EnsureWebView2

  ${If} $Update == 1
    Exec '"$INSTDIR\kybernow.exe" --updated'
  ${EndIf}
SectionEnd

Section "Uninstall"
  Call un.WaitForApp
  ; Liste de la barre des tâches : elle pointerait vers un programme effacé.
  ExecWait '"$INSTDIR\kybernow.exe" --clear-shell'
  MessageBox MB_YESNO|MB_ICONQUESTION "Effacer aussi vos ordinateurs, leurs réglages et leurs mots de passe ?$\r$\n$\r$\nNon : ils seront là si vous réinstallez KyberNow." /SD IDNO IDNO keep
    ; L'app efface ses mots de passe du gestionnaire d'identifiants.
    ExecWait '"$INSTDIR\kybernow.exe" --forget-all'
    RMDir /r "$APPDATA\KyberNow"
    RMDir /r "$LOCALAPPDATA\KyberNow"
    ; Profil de WebView2 (interface), sous l'identifiant de l'app Tauri.
    RMDir /r "$LOCALAPPDATA\fr.leslignesclaires.kybernow"
  keep:
  Delete "$SMPROGRAMS\KyberNow.lnk"
  RMDir /r "$INSTDIR"
  DeleteRegKey HKCU "${UNINST}"
  DeleteRegKey HKCU "Software\Classes\kybernow"
  DeleteRegKey HKCU "Software\Classes\AppUserModelId\LesLignesClaires.KyberNow"
SectionEnd
