# Mise a jour de KyberNow sur cet ordinateur, sans reinstaller ni changer
# les ports : Kyber corrige (surcouche), console, icone de la zone de
# notification, et ce qui va avec (droits des services, haut-parleurs
# virtuels de Steam, pare-feu).
#
# Trois facons de la lancer :
#   - depuis la console (Mises a jour > Installer) : elle telecharge le
#     paquet, verifie son empreinte, puis lance ce script avec -Source ;
#   - a la main, dans PowerShell en administrateur, derniere version publiee :
#       irm https://github.com/MonsieurNours/kybernow/releases/latest/download/update.ps1 | iex
#   - pour tester un paquet servi depuis le Mac : definir KYBERNOW_UPDATE_BASE
#     (ex. http://192.168.1.105:8000) avant la commande ci-dessus.
# Fichier en ASCII : PowerShell 5 lit les scripts telecharges en ANSI.
param([string]$Source)

$ErrorActionPreference = 'Stop'
$base = if ($env:KYBERNOW_UPDATE_BASE) { $env:KYBERNOW_UPDATE_BASE } else { 'https://github.com/MonsieurNours/kybernow/releases/latest/download' }
$dir = 'C:\Program Files\KyberNow'
$data = Join-Path $env:ProgramData 'KyberNow'
$tray = Join-Path $dir 'tray\kybernow-tray.exe'
# Lance par la console (compte SYSTEME) : pas de session a l'ecran, pas de HKCU utile.
$asSystem = [Security.Principal.WindowsIdentity]::GetCurrent().IsSystem
# Droits Windows par defaut, plus demarrer/arreter (RP, WP, DT) pour les utilisateurs interactifs (IU).
$sddl = 'D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWRPWPDTLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)S:(AU;FA;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;WD)'

if ($Source) {
    $tmp = $Source
} else {
    Write-Host '==> Telechargement'
    $tmp = Join-Path $env:TEMP 'kybernow-update'
    Remove-Item $tmp, "$tmp.zip" -Recurse -Force -ErrorAction SilentlyContinue
    Invoke-WebRequest "$base/kybernow-update.zip" -OutFile "$tmp.zip"
    Expand-Archive "$tmp.zip" $tmp -Force
}

Write-Host '==> Arret de l''icone et des services'
$trayWasRunning = [bool](Get-Process kybernow-tray -ErrorAction SilentlyContinue)
Get-Process kybernow-tray -ErrorAction SilentlyContinue | Stop-Process -Force
Stop-Service kyber-admin, kyber -Force
Get-Process kyavserver, kycontroller, kynputserver, kyber-admin -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2

Write-Host '==> Copie (les fichiers Kyber remplaces sont gardes dans kyber\avant-maj)'
$backup = Join-Path $dir 'kyber\avant-maj'
New-Item -ItemType Directory -Force $backup | Out-Null
Get-ChildItem "$tmp\kyber" | ForEach-Object {
    $old = Join-Path "$dir\kyber" $_.Name
    if ((Test-Path $old) -and -not (Test-Path (Join-Path $backup $_.Name))) { Copy-Item $old $backup }
}
Copy-Item "$tmp\kyber\*" "$dir\kyber\" -Force
Copy-Item "$tmp\admin\kyber-admin.exe" "$dir\admin\" -Force
New-Item -ItemType Directory -Force (Join-Path $dir 'tray') | Out-Null
# L'icone et le chargeur WebView2 de son panneau.
Copy-Item "$tmp\tray\*" (Join-Path $dir 'tray') -Force

Write-Host '==> Droits des services (l''icone les pilote sans droits administrateur)'
foreach ($s in 'kyber', 'kyber-admin') {
    sc.exe sdset $s $sddl | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "sc sdset $s a echoue (code $LASTEXITCODE)" }
}

Write-Host '==> Haut-parleurs virtuels de Steam'
$speakers = Get-PnpDevice -FriendlyName '*Steam Streaming Speakers*' -ErrorAction SilentlyContinue
$inf = @(
    "${env:CommonProgramFiles(x86)}\Steam\drivers\Windows10\x64\SteamStreamingSpeakers.inf",
    "${env:CommonProgramFiles(x86)}\Steam\drivers\Windows\x64\SteamStreamingSpeakers.inf"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($speakers) {
    Write-Host '    deja installes'
} elseif ($inf) {
    pnputil /add-driver "$inf" /install
} else {
    Write-Warning 'Steam absent : le son restera sur la sortie de Windows.'
}

Write-Host '==> Menu Demarrer'
$lnk = Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu\Programs\KyberNow\KyberNow.lnk'
New-Item -ItemType Directory -Force (Split-Path $lnk) | Out-Null
$shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
$shortcut.TargetPath = $tray
$shortcut.Description = 'KyberNow dans la zone de notification'
$shortcut.Save()
if (-not $asSystem) {
    # Lancement a l'ouverture de session (HKCU de l'utilisateur, pas celui du systeme).
    $run = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
    if ((Get-Service kyber).StartType -eq 'Automatic') {
        Set-ItemProperty $run -Name KyberNow -Value "`"$tray`""
    } else {
        Remove-ItemProperty $run -Name KyberNow -ErrorAction SilentlyContinue
    }
}

Write-Host '==> Pare-feu : annonce Bonjour de la console (UDP 5353)'
netsh advfirewall firewall delete rule name=KyberNow-bonjour-UDP | Out-Null
netsh advfirewall firewall add rule name=KyberNow-bonjour-UDP dir=in action=allow protocol=UDP localport=5353 "program=$dir\admin\kyber-admin.exe" profile=private,domain | Out-Null

if ($asSystem -and $trayWasRunning) {
    # La nouvelle console relance l'icone dans la session de l'utilisateur.
    New-Item -ItemType Directory -Force (Join-Path $data 'update') | Out-Null
    Set-Content -Path (Join-Path $data 'update\relaunch-tray') -Value '' -Encoding ascii
}

Write-Host '==> Redemarrage des services'
Start-Service kyber, kyber-admin

if (-not $asSystem) {
    Write-Host '==> Lancement de l''icone (sans droits administrateur)'
    Start-Process explorer.exe -ArgumentList "`"$tray`""
}
Write-Host 'Mise a jour terminee.'
