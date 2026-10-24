# Mise a jour de KyberNow Station sur cet ordinateur, sans reinstaller ni
# changer les ports : Kyber corrige (surcouche), KyberNow Admin (la console),
# icone de la zone de notification, et ce qui va avec (droits des services,
# haut-parleurs virtuels de Steam, pare-feu, raccourcis du menu Demarrer et
# noms affiches des services).
#
# La console en 0.2.6 et apres n'utilise pas ce script : son applicateur
# (kyber-admin update apply) verifie le manifeste signe de la version et fait
# la bascule lui-meme. Ce script sert a :
#   - les consoles 0.2.5 et plus anciennes, qui le lancent avec -Source apres
#     avoir verifie l'empreinte du paquet (compatibilite : c'est leur seul
#     chemin de mise a jour) ;
#   - la mise a jour a la main, dans PowerShell en administrateur, derniere
#     version publiee :
#       irm https://github.com/MonsieurNours/kybernow/releases/latest/download/update.ps1 | iex
#     Integrite seulement : SHA256SUMS dit que le paquet n'est pas abime, pas
#     qu'il vient de KyberNow (il est publie au meme endroit). Voir
#     docs/releases.md ;
#   - pour tester un paquet servi depuis le Mac : definir KYBERNOW_UPDATE_BASE
#     (ex. http://192.168.1.105:8000) avant la commande ci-dessus.
#
# Comme Admin, le script refuse, avant d'arreter quoi que ce soit, une
# version qui exige l'installateur ou une Station trop ancienne pour le
# paquet (requirements.json du paquet).
#
# Bascule sans etat intermediaire : les nouveaux fichiers sont prepares dans
# kyber.new, admin.new et tray.new, puis chaque dossier est renomme
# (kyber -> kyber.prev, kyber.new -> kyber). Apres le redemarrage, la console
# doit repondre sur son port en 60 s ; sinon les dossiers precedents sont
# remis en place et la raison est ecrite dans le journal et dans
# C:\ProgramData\KyberNow\update\resultat.json (lu par la console). Quoi qu'il
# arrive, les services et l'icone sont relances a la fin.
# Fichier en ASCII : PowerShell 5 lit les scripts telecharges en ANSI.
param([string]$Source)

$ErrorActionPreference = 'Stop'
# Sans barre de progression : Invoke-WebRequest et Expand-Archive de
# PowerShell 5 en sont plusieurs fois plus lents.
$ProgressPreference = 'SilentlyContinue'
$base = if ($env:KYBERNOW_UPDATE_BASE) { $env:KYBERNOW_UPDATE_BASE } else { 'https://github.com/MonsieurNours/kybernow/releases/latest/download' }
$dir = Join-Path $env:ProgramFiles 'KyberNow'
$data = Join-Path $env:ProgramData 'KyberNow'
$updates = Join-Path $data 'update'
$tray = Join-Path $dir 'tray\kybernow-tray.exe'
$uninstallKey = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\KyberNow'
# Lance par la console (compte SYSTEME) : pas de session a l'ecran, pas de HKCU utile.
$asSystem = [Security.Principal.WindowsIdentity]::GetCurrent().IsSystem
# Droits Windows par defaut, plus demarrer et arreter (RP, WP) pour les utilisateurs interactifs (IU).
$sddl = 'D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWRPWPLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)S:(AU;FA;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;WD)'
# Dossiers remplaces, dans l'ordre de la bascule.
$parts = 'kyber', 'admin', 'tray'
$kyberProcesses = 'kyavserver', 'kycontroller', 'kynputserver', 'kyclient', 'kyservice', 'kyber-admin'

# Ecrit un fichier texte en UTF-8 sans BOM (serde_json refuse le BOM de Set-Content).
function Write-Utf8([string]$Path, [string]$Text) {
    [IO.File]::WriteAllText($Path, $Text, (New-Object Text.UTF8Encoding $false))
}

# Renomme en reessayant : un antivirus ou une fenetre de l'Explorateur peut
# garder un fichier ouvert quelques secondes.
function Rename-Retry([string]$Path, [string]$NewName) {
    for ($i = 1; ; $i++) {
        try {
            Rename-Item -LiteralPath $Path -NewName $NewName
            return
        } catch {
            if ($i -ge 10) { throw "renommage de $Path en $NewName impossible : $($_.Exception.Message)" }
            Start-Sleep -Seconds 1
        }
    }
}

function Remove-Dir([string]$Path) {
    if (Test-Path -LiteralPath $Path) { Remove-Item -LiteralPath $Path -Recurse -Force }
}

# Arrete le serveur puis la console, et ce qui pourrait tenir leurs fichiers ouverts.
function Stop-KyberNow {
    foreach ($s in 'kyber', 'kyber-admin') {
        if (Get-Service $s -ErrorAction SilentlyContinue) { Stop-Service $s -Force -ErrorAction SilentlyContinue }
    }
    Get-Process $kyberProcesses -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
}

# Port de la console : kyber-admin.toml (listen), sinon kybernow.json, sinon 9443.
function Get-ConsolePort {
    $toml = Join-Path $data 'kyber-admin.toml'
    if (Test-Path $toml) {
        foreach ($line in Get-Content $toml) {
            if ($line -match '^\s*listen\s*=\s*"[^"]*:(\d+)"') { return [int]$Matches[1] }
        }
    }
    $json = Join-Path $dir 'kybernow.json'
    if (Test-Path $json) {
        $record = Get-Content -Raw -Encoding UTF8 $json | ConvertFrom-Json
        if ($record.console_port) { return [int]$record.console_port }
    }
    return 9443
}

# Controle de sante : la console accepte une connexion sur son port avant l'echeance.
function Test-Console([int]$Port, [int]$Seconds) {
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline) {
        $client = New-Object System.Net.Sockets.TcpClient
        try {
            if ($client.ConnectAsync('127.0.0.1', $Port).Wait(2000) -and $client.Connected) { return $true }
        } catch {
            # refus de connexion : la console demarre encore
        } finally {
            $client.Close()
        }
        Start-Sleep -Seconds 2
    }
    return $false
}

# Version du paquet : version.txt (paquets 0.2.6 et suivants), sinon celle de la console.
function Get-PackageVersion([string]$Package) {
    $file = Join-Path $Package 'version.txt'
    $text = if (Test-Path $file) { (Get-Content -Raw $file).Trim() } else { [string](& (Join-Path $Package 'admin\kyber-admin.exe') --version) }
    if ($text -match '(\d+\.\d+\.\d+)') { return $Matches[1] }
    throw "version du paquet illisible ($text)"
}

# Version de KyberNow Station installee ici : kybernow.json (installateur et
# mises a jour), sinon celle de la console installee. Vide si illisible.
function Get-InstalledVersion {
    $json = Join-Path $dir 'kybernow.json'
    if (Test-Path $json) {
        try {
            $record = Get-Content -Raw -Encoding UTF8 $json | ConvertFrom-Json
            if ([string]$record.version -match '^\d+\.\d+\.\d+$') { return [string]$record.version }
        } catch {
            # kybernow.json abime : la console installee le dit
        }
    }
    $exe = Join-Path $dir 'admin\kyber-admin.exe'
    if (Test-Path $exe) {
        try {
            if ([string](& $exe --version) -match '(\d+\.\d+\.\d+)') { return $Matches[1] }
        } catch {
            # console absente ou qui ne se lance pas
        }
    }
    return ''
}

# Exigences du paquet (requirements.json, paquets 0.2.6 et suivants) : copie
# de celles du manifeste signe, qu'Admin verifie avant d'installer en un clic.
# Une version qui exige l'installateur, ou une Station installee avant
# min_installer_version, est refusee avant d'arreter quoi que ce soit. Paquet
# sans ce fichier (plus ancien) : rien a verifier.
function Assert-Requirements([string]$Package, [string]$Version) {
    $file = Join-Path $Package 'requirements.json'
    if (-not (Test-Path $file)) { return }
    $setup = 'KyberNow-Station-Setup.exe'
    try {
        $req = Get-Content -Raw -Encoding UTF8 $file | ConvertFrom-Json
    } catch {
        throw "requirements.json illisible dans le paquet : $($_.Exception.Message)"
    }
    if ($req.requires_setup -eq $true) { throw "la version $Version s'installe avec $setup : reinstallez avec lui" }
    $min = [string]$req.min_installer_version
    if (-not $min) { return }
    if ($min -notmatch '^\d+\.\d+\.\d+$') { throw "requirements.json : version minimale inattendue ($min)" }
    $installed = Get-InstalledVersion
    if (-not $installed) { throw "version de KyberNow Station installee illisible : reinstallez avec $setup" }
    if ([version]$min -gt [version]$installed) { throw "la version $Version demande au moins la $min (installee : $installed) : reinstallez avec $setup" }
}

# Langue de l'installation : celle que l'installateur a donnee au nom affiche
# du service kyber (langue de Windows a l'installation), sinon celle de Windows.
function Test-French {
    $current = [string](Get-Service kyber -ErrorAction SilentlyContinue).DisplayName
    if ($current -match 'serveur') { return $true }
    if ($current -match 'server') { return $false }
    return ((Get-UICulture).Name -like 'fr*')
}

# Noms affiches de KyberNow Station : services (le nom technique kyber /
# kyber-admin ne change pas) et Applications installees (la cle Uninstall\KyberNow
# non plus). L'installateur les pose a la creation des services ; une mise a
# jour ne recree pas les services, d'ou ce rattrapage. Jamais bloquant.
function Set-DisplayNames([bool]$French) {
    $eacute = [char]0xE9
    if ($French) {
        $kyberName = 'KyberNow Station - serveur de streaming'
        $kyberDescription = "Serveur de streaming Kyber, install${eacute} et g${eacute}r${eacute} par KyberNow Station."
    } else {
        $kyberName = 'KyberNow Station - streaming server'
        $kyberDescription = 'Kyber streaming server, installed and managed by KyberNow Station.'
    }
    sc.exe config kyber DisplayName= $kyberName | Out-Null
    if ($LASTEXITCODE -ne 0) { Write-Warning "nom affiche du service kyber inchange (code $LASTEXITCODE)" }
    sc.exe description kyber $kyberDescription | Out-Null
    sc.exe config kyber-admin DisplayName= 'KyberNow Admin' | Out-Null
    if ($LASTEXITCODE -ne 0) { Write-Warning "nom affiche du service kyber-admin inchange (code $LASTEXITCODE)" }
    if (Test-Path $uninstallKey) { Set-ItemProperty $uninstallKey -Name DisplayName -Value 'KyberNow Station' }
}

# Demande a la console qui demarre de relancer l'icone dans la session de l'utilisateur.
function Request-TrayRelaunch {
    New-Item -ItemType Directory -Force $updates | Out-Null
    Set-Content -Path (Join-Path $updates 'relaunch-tray') -Value '' -Encoding ascii
}

# Resultat lu par la console a son demarrage (Mises a jour : echec et retour arriere).
function Write-Result([string]$Version, [string]$Message, [bool]$RolledBack) {
    New-Item -ItemType Directory -Force $updates | Out-Null
    $log = if ($Source) { Join-Path (Split-Path $Source) 'update.log' } else { '' }
    $result = [ordered]@{ version = $Version; ok = $false; rolled_back = $RolledBack; message = $Message; log = $log }
    Write-Utf8 (Join-Path $updates 'resultat.json') ($result | ConvertTo-Json)
}

$version = ''
$pkg = $null
$stopped = $false
$trayWasRunning = $false
$swapped = New-Object System.Collections.ArrayList
$failure = $null
$rolledBack = $false

try {
    if ($Source) {
        $pkg = $Source
    } else {
        Write-Host '==> Telechargement'
        $pkg = Join-Path $env:TEMP 'kybernow-update'
        Remove-Item $pkg, "$pkg.zip", "$pkg.sums" -Recurse -Force -ErrorAction SilentlyContinue
        Invoke-WebRequest "$base/kybernow-update.zip" -OutFile "$pkg.zip" -UseBasicParsing
        Invoke-WebRequest "$base/SHA256SUMS" -OutFile "$pkg.sums" -UseBasicParsing
        Write-Host '==> Verification de l''empreinte (SHA256SUMS)'
        $expected = $null
        foreach ($line in Get-Content "$pkg.sums") {
            if ($line -match '^([0-9a-fA-F]{64})\s+\*?kybernow-update\.zip\s*$') { $expected = $Matches[1].ToLower() }
        }
        if (-not $expected) { throw 'empreinte du paquet absente de SHA256SUMS' }
        $actual = (Get-FileHash "$pkg.zip" -Algorithm SHA256).Hash.ToLower()
        if ($actual -ne $expected) { throw 'empreinte du paquet incorrecte : telechargement abime ou falsifie' }
        Write-Host '    integrite seulement : la signature de la version est verifiee par la console, pas par ce script'
        Expand-Archive "$pkg.zip" $pkg -Force
    }
    foreach ($p in $parts) {
        if (-not (Test-Path (Join-Path $pkg $p))) { throw "paquet incomplet : dossier $p absent" }
    }
    $version = Get-PackageVersion $pkg
    Write-Host "==> KyberNow Station $version"
    Assert-Requirements $pkg $version
    $french = Test-French

    Write-Host '==> Arret du serveur Kyber, de KyberNow Admin, puis de l''icone'
    $stopped = $true
    Stop-KyberNow
    $trayWasRunning = [bool](Get-Process kybernow-tray -ErrorAction SilentlyContinue)
    Get-Process kybernow-tray -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Seconds 1

    Write-Host '==> Preparation des nouveaux dossiers (kyber.new, admin.new, tray.new)'
    foreach ($p in $parts) {
        $cur = Join-Path $dir $p
        Remove-Dir "$cur.new"
        Remove-Dir "$cur.prev"
        Remove-Dir "$cur.bad"
        New-Item -ItemType Directory -Force "$cur.new" | Out-Null
        if (Test-Path $cur) {
            # Tout le dossier actuel (reglages et certificat de Kyber compris),
            # sauf les journaux de Kyber, deplaces apres la bascule, et le
            # dossier avant-maj des anciennes mises a jour (la version
            # precedente est gardee dans kyber.prev).
            Get-ChildItem -LiteralPath $cur -Force | Where-Object { -not ($p -eq 'kyber' -and $_.Name -in 'log', 'avant-maj') } |
                ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination "$cur.new" -Recurse -Force }
        }
        # Puis les fichiers du paquet par-dessus.
        Copy-Item (Join-Path $pkg "$p\*") "$cur.new" -Recurse -Force
    }

    Write-Host '==> Bascule (les dossiers precedents restent en .prev)'
    foreach ($p in $parts) {
        $cur = Join-Path $dir $p
        $had = Test-Path $cur
        if ($had) { Rename-Retry $cur "$p.prev" }
        [void]$swapped.Add([pscustomobject]@{ Name = $p; HadPrevious = $had })
        Rename-Retry "$cur.new" $p
    }
    # Journaux de Kyber : deplaces, pas copies.
    $prevLog = Join-Path $dir 'kyber.prev\log'
    if ((Test-Path $prevLog) -and -not (Test-Path (Join-Path $dir 'kyber\log'))) { Move-Item -LiteralPath $prevLog (Join-Path $dir 'kyber\log') }

    Write-Host '==> Droits des services (l''icone les demarre et les arrete sans droits administrateur)'
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
    # Le dossier KyberNow reste. Raccourcis des versions 0.3.1 et avant :
    # KyberNow.lnk -> KyberNow Station.lnk (icone), Console KyberNow.url et
    # KyberNow console.url -> KyberNow Admin.url (meme adresse, ecrite par
    # l'installateur avec le port de la console).
    $menu = Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu\Programs\KyberNow'
    New-Item -ItemType Directory -Force $menu | Out-Null
    $adminUrl = Join-Path $menu 'KyberNow Admin.url'
    foreach ($old in 'Console KyberNow.url', 'KyberNow console.url') {
        $oldUrl = Join-Path $menu $old
        if (-not (Test-Path -LiteralPath $oldUrl)) { continue }
        if (Test-Path -LiteralPath $adminUrl) {
            Remove-Item -LiteralPath $oldUrl -Force -ErrorAction SilentlyContinue
        } else {
            Move-Item -LiteralPath $oldUrl -Destination $adminUrl -ErrorAction SilentlyContinue
        }
    }
    Remove-Item -LiteralPath (Join-Path $menu 'KyberNow.lnk') -Force -ErrorAction SilentlyContinue
    $lnk = Join-Path $menu 'KyberNow Station.lnk'
    $shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
    $shortcut.TargetPath = $tray
    $trayDescription = if ($french) { 'KyberNow Station dans la zone de notification' } else { 'KyberNow Station in the notification area' }
    $shortcut.Description = $trayDescription
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

    Write-Host '==> Pare-feu : annonce Bonjour de KyberNow Admin (UDP 5353)'
    netsh advfirewall firewall delete rule name=KyberNow-bonjour-UDP | Out-Null
    netsh advfirewall firewall add rule name=KyberNow-bonjour-UDP dir=in action=allow protocol=UDP localport=5353 "program=$dir\admin\kyber-admin.exe" profile=private,domain | Out-Null

    # La nouvelle console relance l'icone dans la session de l'utilisateur
    # (elle lit ce fichier a son demarrage).
    if ($asSystem -and $trayWasRunning) { Request-TrayRelaunch }

    # Un echec precedent ne doit pas s'afficher dans la nouvelle console.
    Remove-Item (Join-Path $updates 'resultat.json') -Force -ErrorAction SilentlyContinue

    Write-Host '==> Redemarrage et controle de sante (KyberNow Admin doit repondre en 60 s)'
    Start-Service kyber-admin, kyber
    $port = Get-ConsolePort
    if (-not (Test-Console $port 60)) { throw "la nouvelle console ne repond pas sur le port $port apres 60 s" }
    if ((Get-Service kyber).Status -ne 'Running') { throw 'le serveur Kyber ne demarre pas' }
    Write-Host "    KyberNow Admin en ligne sur le port $port"

    # La nouvelle version tourne : la suite ne doit plus la defaire.
    $swapped.Clear()
    Write-Host '==> Version enregistree'
    try {
        $json = Join-Path $dir 'kybernow.json'
        if (Test-Path $json) {
            $record = Get-Content -Raw -Encoding UTF8 $json | ConvertFrom-Json
            $record.version = $version
            Write-Utf8 $json ($record | ConvertTo-Json -Depth 5)
        }
        if (Test-Path $uninstallKey) { Set-ItemProperty $uninstallKey -Name DisplayVersion -Value $version }
    } catch {
        Write-Warning "version non enregistree ($($_.Exception.Message))"
    }
    Write-Host '==> Noms affiches (KyberNow Station, KyberNow Admin)'
    try {
        Set-DisplayNames $french
    } catch {
        Write-Warning "noms affiches inchanges ($($_.Exception.Message))"
    }
    Remove-Item (Join-Path $updates 'resultat.json') -Force -ErrorAction SilentlyContinue
} catch {
    $failure = $_.Exception.Message
    Write-Host "ERREUR : $failure"
    Write-Host $_.ScriptStackTrace
    if ($swapped.Count -gt 0) {
        Write-Host '==> Retour a la version precedente'
        try {
            Stop-KyberNow
            Get-Process kybernow-tray -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
            $kyberLog = Join-Path $dir 'kyber\log'
            if ((Test-Path $kyberLog) -and (Test-Path (Join-Path $dir 'kyber.prev')) -and -not (Test-Path (Join-Path $dir 'kyber.prev\log'))) {
                Move-Item -LiteralPath $kyberLog (Join-Path $dir 'kyber.prev\log')
            }
            for ($i = $swapped.Count - 1; $i -ge 0; $i--) {
                $p = $swapped[$i].Name
                $cur = Join-Path $dir $p
                Remove-Dir "$cur.bad"
                if (Test-Path $cur) { Rename-Retry $cur "$p.bad" }
                if ($swapped[$i].HadPrevious) { Rename-Retry "$cur.prev" $p }
                Remove-Item -LiteralPath "$cur.bad" -Recurse -Force -ErrorAction SilentlyContinue
            }
            $rolledBack = $true
            Write-Host '    version precedente remise en place'
        } catch {
            Write-Host "ERREUR pendant le retour arriere : $($_.Exception.Message)"
            Write-Host "Dossiers a verifier dans $dir (kyber, admin, tray et leurs .prev, .bad)."
        }
    }
    # La console a ete arretee : celle qui redemarre lit le resultat. Sinon,
    # elle tourne encore et lit la fin du journal elle-meme.
    if ($stopped) {
        try { Write-Result $version $failure $rolledBack } catch { Write-Host "resultat.json : $($_.Exception.Message)" }
    }
} finally {
    foreach ($p in $parts) { Remove-Item -LiteralPath (Join-Path $dir "$p.new") -Recurse -Force -ErrorAction SilentlyContinue }
    if ($stopped) {
        # Echec : la console qui redemarre (l'ancienne, remise en place) relance l'icone.
        if ($failure -and $asSystem -and $trayWasRunning) { Request-TrayRelaunch }
        Write-Host '==> Demarrage des services'
        foreach ($s in 'kyber-admin', 'kyber') {
            try { Start-Service $s } catch { Write-Host "    $s : $($_.Exception.Message)" }
        }
        if (-not $asSystem) {
            Write-Host '==> Lancement de l''icone (sans droits administrateur)'
            Start-Process explorer.exe -ArgumentList "`"$tray`""
        }
    }
    # Mise a jour a la main : le paquet telecharge ne sert plus.
    if (-not $Source -and $pkg) { Remove-Item $pkg, "$pkg.zip", "$pkg.sums" -Recurse -Force -ErrorAction SilentlyContinue }
    # Paquets des mises a jour : les deux plus recents restent.
    if (Test-Path $updates) {
        Get-ChildItem $updates -Directory | Where-Object { $_.Name -match '^\d+\.\d+\.\d+$' } |
            Sort-Object { [version]$_.Name } -Descending | Select-Object -Skip 2 |
            Where-Object { -not ($Source -and $Source.StartsWith($_.FullName, [StringComparison]::OrdinalIgnoreCase)) } |
            ForEach-Object { Remove-Item -LiteralPath $_.FullName -Recurse -Force -ErrorAction SilentlyContinue }
    }
}

if ($failure) {
    $what = if ($rolledBack) { 'version precedente remise en place' } else { 'rien n''a ete remplace' }
    throw "Mise a jour non installee ($what) : $failure"
}
Write-Host "Mise a jour terminee : KyberNow Station $version."
