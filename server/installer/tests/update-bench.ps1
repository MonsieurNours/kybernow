# Banc d'essai de update.ps1, hors Windows (pwsh sur le Mac : brew install powershell).
#   pwsh -NoProfile -File server/installer/tests/update-bench.ps1
# Services, processus et commandes de Windows sont simules ; les dossiers sont
# reels, dans un dossier temporaire. Le controle de sante est une vraie
# connexion TCP : la console simulee ouvre son port, sauf si son exe contient
# le mot 'broken'. Scenarios : mise a jour reussie (deux fois de suite),
# console muette (retour arriere), paquet incomplet, renommage bloque, paquet
# qui exige l'installateur ou une Station plus recente (requirements.json).
param([string]$Script = (Join-Path $PSScriptRoot '../update.ps1'))

$ErrorActionPreference = 'Stop'
$failures = 0
function Check([bool]$cond, [string]$what) {
    if ($cond) { Write-Host "  ok   $what" -ForegroundColor Green }
    else { Write-Host "  FAIL $what" -ForegroundColor Red; $script:failures++ }
}

$text = Get-Content -Raw $Script
$text = $text.Replace('[Security.Principal.WindowsIdentity]::GetCurrent().IsSystem', '$true')
$text = $text.Replace('(New-Object -ComObject WScript.Shell)', '(Get-FakeShell)')
$text = $text.Replace('Test-Console $port 60', 'Test-Console $port 4')
$copy = Join-Path ([IO.Path]::GetTempPath()) 'update-under-test.ps1'
Set-Content $copy $text

$global:svc = @{}
$global:listener = $null
$global:port = 0
$global:renameFail = $null

function global:Get-FakeShell { [pscustomobject]@{} | Add-Member -MemberType ScriptMethod -Name CreateShortcut -Value { param($p) [pscustomobject]@{ TargetPath = ''; Description = '' } | Add-Member -MemberType ScriptMethod -Name Save -Value { } -PassThru } -PassThru }
function global:Get-Service { [CmdletBinding()] param([string]$Name) if ($global:svc.ContainsKey($Name)) { [pscustomobject]@{ Name = $Name; Status = $global:svc[$Name]; StartType = 'Automatic' } } }
function global:Stop-Service { [CmdletBinding()] param([Parameter(Position = 0)][string[]]$Name, [switch]$Force) foreach ($n in $Name) { $global:svc[$n] = 'Stopped'; if ($n -eq 'kyber-admin' -and $global:listener) { $global:listener.Stop(); $global:listener = $null } } }
function global:Start-Service([string[]]$Name) {
    foreach ($n in $Name) {
        if ($n -eq 'kyber-admin') {
            $exe = Join-Path $env:ProgramFiles 'KyberNow/admin/kyber-admin.exe'
            if ((Get-Content -Raw $exe) -match 'broken') { $global:svc[$n] = 'Stopped'; continue }
            if (-not $global:listener) {
                $global:listener = [System.Net.Sockets.TcpListener]::new([ipaddress]::Loopback, $global:port)
                $global:listener.Start()
            }
        }
        $global:svc[$n] = 'Running'
    }
}
function global:Get-Process { [CmdletBinding()] param([Parameter(Position = 0)][string[]]$Name) if ($Name -contains 'kybernow-tray') { [pscustomobject]@{ Name = 'kybernow-tray' } } }
function global:Stop-Process { param([Parameter(ValueFromPipeline)]$p, [switch]$Force) process { } }
function global:Start-Sleep { [CmdletBinding()] param([Parameter(Position = 0)]$Seconds, $Milliseconds) }
function global:sc.exe { $global:LASTEXITCODE = 0 }
function global:netsh { }
function global:pnputil { }
function global:Get-PnpDevice { [CmdletBinding()] param($FriendlyName) [pscustomobject]@{} }
function global:Start-Process { }
function global:Rename-Item {
    param([string]$LiteralPath, [string]$NewName)
    if ($global:renameFail -and $LiteralPath.EndsWith($global:renameFail)) { throw "fichier ouvert (simulation)" }
    Microsoft.PowerShell.Management\Rename-Item -LiteralPath $LiteralPath -NewName $NewName
}

function New-Sandbox([string]$AdminContent, [switch]$NoTrayInPackage, [string]$MinInstaller = '0.2.5', [string]$RequiresSetup = 'false') {
    $root = Join-Path ([IO.Path]::GetTempPath()) ("kn-" + [guid]::NewGuid())
    $env:ProgramFiles = Join-Path $root 'pf'
    $env:ProgramData = Join-Path $root 'pd'
    $dir = Join-Path $env:ProgramFiles 'KyberNow'
    $data = Join-Path $env:ProgramData 'KyberNow'
    foreach ($d in 'kyber/log', 'kyber/avant-maj', 'admin', 'tray') { New-Item -ItemType Directory -Force (Join-Path $dir $d) | Out-Null }
    Set-Content (Join-Path $dir 'kyber/kyber_config.toml') 'reglages'
    Set-Content (Join-Path $dir 'kyber/kyavserver.exe') 'ancien'
    Set-Content (Join-Path $dir 'kyber/cert.pem') 'certificat'
    Set-Content (Join-Path $dir 'kyber/log/kyber.log') 'journal'
    Set-Content (Join-Path $dir 'kyber/avant-maj/kyavserver.exe') 'origine'
    Set-Content (Join-Path $dir 'admin/kyber-admin.exe') 'ancienne console'
    Set-Content (Join-Path $dir 'tray/kybernow-tray.exe') 'ancienne icone'
    $global:port = Get-Random -Minimum 30000 -Maximum 40000
    Set-Content (Join-Path $dir 'kybernow.json') ('{"version":"0.2.5","kyber_version":"0.29.0","stream_port":1,"console_port":' + $global:port + ',"virtual_display":true,"gamepads":true,"sudovda_thumbprint":null}')
    foreach ($v in '0.2.3', '0.2.4', '0.2.5') { New-Item -ItemType Directory -Force (Join-Path $data "update/$v") | Out-Null }
    $pkg = Join-Path $data 'update/0.2.6/contenu'
    foreach ($d in 'kyber', 'admin', 'tray') { if (-not ($NoTrayInPackage -and $d -eq 'tray')) { New-Item -ItemType Directory -Force (Join-Path $pkg $d) | Out-Null } }
    Set-Content (Join-Path $pkg 'kyber/kyavserver.exe') 'nouveau'
    Set-Content (Join-Path $pkg 'admin/kyber-admin.exe') $AdminContent
    if (-not $NoTrayInPackage) { Set-Content (Join-Path $pkg 'tray/kybernow-tray.exe') 'nouvelle icone' }
    Set-Content (Join-Path $pkg 'version.txt') '0.2.6'
    Set-Content (Join-Path $pkg 'requirements.json') ('{"version": "0.2.6", "min_installer_version": "' + $MinInstaller + '", "requires_setup": ' + $RequiresSetup + '}')
    $global:svc = @{ 'kyber' = 'Running'; 'kyber-admin' = 'Running' }
    return [pscustomobject]@{ Dir = $dir; Data = $data; Pkg = $pkg; Root = $root }
}

function Read-File([string]$Path) { if (Test-Path $Path) { Get-Content -Raw $Path } else { '<absent>' } }
function Invoke-Update($sb) {
    try { & $copy -Source $sb.Pkg *> $null; return $null } catch { return $_.Exception.Message }
}

Write-Host '1. Mise a jour reussie'
$sb = New-Sandbox 'nouvelle console'
$err = Invoke-Update $sb
Check ($null -eq $err) "pas d'erreur ($err)"
Check ((Read-File "$($sb.Dir)/kyber/kyavserver.exe") -match 'nouveau') 'kyavserver remplace'
Check ((Read-File "$($sb.Dir)/kyber/kyber_config.toml") -match 'reglages') 'kyber_config.toml garde'
Check ((Read-File "$($sb.Dir)/kyber/cert.pem") -match 'certificat') 'certificat garde'
Check (Test-Path "$($sb.Dir)/kyber/log/kyber.log") 'journaux de Kyber deplaces'
Check (-not (Test-Path "$($sb.Dir)/kyber/avant-maj")) 'avant-maj non recopie'
Check ((Read-File "$($sb.Dir)/kyber.prev/kyavserver.exe") -match 'ancien') 'kyber.prev garde l''ancien'
Check ((Read-File "$($sb.Dir)/admin/kyber-admin.exe") -match 'nouvelle') 'console remplacee'
Check ((Read-File "$($sb.Dir)/tray/kybernow-tray.exe") -match 'nouvelle') 'icone remplacee'
Check (-not (Test-Path "$($sb.Dir)/kyber.new")) 'pas de kyber.new'
$json = Get-Content -Raw "$($sb.Dir)/kybernow.json"
Check (($json | ConvertFrom-Json).version -eq '0.2.6') 'kybernow.json : version 0.2.6'
Check ([IO.File]::ReadAllBytes("$($sb.Dir)/kybernow.json")[0] -eq [byte][char]'{') 'kybernow.json sans BOM'
Check (-not (Test-Path "$($sb.Data)/update/resultat.json")) 'pas de resultat.json'
Check (Test-Path "$($sb.Data)/update/relaunch-tray") 'relance de l''icone demandee'
$left = (Get-ChildItem "$($sb.Data)/update" -Directory).Name -join ','
Check ($left -eq '0.2.5,0.2.6') "purge des paquets ($left)"
Check ($global:svc['kyber'] -eq 'Running' -and $global:svc['kyber-admin'] -eq 'Running') 'services demarres'

Write-Host '1 bis. Deuxieme mise a jour : kyber.prev remplace'
Set-Content (Join-Path $sb.Pkg 'kyber/kyavserver.exe') 'encore plus nouveau'
$err = Invoke-Update $sb
Check ($null -eq $err) "pas d'erreur ($err)"
Check ((Read-File "$($sb.Dir)/kyber.prev/kyavserver.exe") -match '^nouveau') 'kyber.prev = version precedente'
Check (Test-Path "$($sb.Dir)/kyber/log/kyber.log") 'journaux toujours la'
if ($global:listener) { $global:listener.Stop(); $global:listener = $null }

Write-Host '2. Controle de sante rate : retour arriere'
$sb = New-Sandbox 'broken'
$err = Invoke-Update $sb
Check ($err -match 'version precedente remise en place') "erreur annoncee ($err)"
Check ((Read-File "$($sb.Dir)/kyber/kyavserver.exe") -match 'ancien') 'kyavserver ancien remis'
Check ((Read-File "$($sb.Dir)/admin/kyber-admin.exe") -match 'ancienne') 'console ancienne remise'
Check ((Read-File "$($sb.Dir)/tray/kybernow-tray.exe") -match 'ancienne') 'icone ancienne remise'
Check (Test-Path "$($sb.Dir)/kyber/log/kyber.log") 'journaux de Kyber revenus'
Check (Test-Path "$($sb.Dir)/kyber/avant-maj") 'avant-maj revenu'
Check (-not (Test-Path "$($sb.Dir)/kyber.prev") -and -not (Test-Path "$($sb.Dir)/admin.bad") -and -not (Test-Path "$($sb.Dir)/kyber.new")) 'ni .prev, ni .bad, ni .new'
$r = Get-Content -Raw "$($sb.Data)/update/resultat.json" | ConvertFrom-Json
Check ($r.ok -eq $false -and $r.rolled_back -eq $true -and $r.version -eq '0.2.6') "resultat.json ($($r.message))"
Check ((Get-Content -Raw "$($sb.Dir)/kybernow.json" | ConvertFrom-Json).version -eq '0.2.5') 'kybernow.json reste en 0.2.5'
Check ($global:svc['kyber-admin'] -eq 'Running') 'ancienne console redemarree'
Check (Test-Path "$($sb.Data)/update/relaunch-tray") 'relance de l''icone demandee'
if ($global:listener) { $global:listener.Stop(); $global:listener = $null }

Write-Host '3. Paquet incomplet : rien n''est arrete'
$sb = New-Sandbox 'nouvelle console' -NoTrayInPackage
$err = Invoke-Update $sb
Check ($err -match 'rien n''a ete remplace' -and $err -match 'tray') "erreur annoncee ($err)"
Check ($global:svc['kyber'] -eq 'Running') 'serveur jamais arrete'
Check (-not (Test-Path "$($sb.Data)/update/resultat.json")) 'pas de resultat.json (la console tourne encore)'
if ($global:listener) { $global:listener.Stop(); $global:listener = $null }

Write-Host '4. Renommage bloque au milieu de la bascule'
$sb = New-Sandbox 'nouvelle console'
$global:renameFail = 'admin.new'
$err = Invoke-Update $sb
$global:renameFail = $null
Check ($err -match 'version precedente remise en place') "erreur annoncee ($err)"
Check ((Read-File "$($sb.Dir)/kyber/kyavserver.exe") -match 'ancien') 'kyber remis'
Check ((Read-File "$($sb.Dir)/admin/kyber-admin.exe") -match 'ancienne') 'admin intact'
Check (-not (Test-Path "$($sb.Dir)/admin.prev") -and -not (Test-Path "$($sb.Dir)/kyber.prev")) 'aucun .prev qui traine'
Check ($global:svc['kyber'] -eq 'Running' -and $global:svc['kyber-admin'] -eq 'Running') 'services redemarres'
if ($global:listener) { $global:listener.Stop(); $global:listener = $null }

Write-Host '5. Version qui exige l''installateur : rien n''est arrete'
$sb = New-Sandbox 'nouvelle console' -RequiresSetup 'true'
$err = Invoke-Update $sb
Check ($err -match 'rien n''a ete remplace' -and $err -match 'KyberNow-Station-Setup\.exe') "erreur annoncee ($err)"
Check ($global:svc['kyber'] -eq 'Running' -and $global:svc['kyber-admin'] -eq 'Running') 'services jamais arretes'
Check ((Read-File "$($sb.Dir)/kyber/kyavserver.exe") -match 'ancien') 'kyber intact'
Check (-not (Test-Path "$($sb.Dir)/kyber.prev") -and -not (Test-Path "$($sb.Dir)/kyber.new")) 'ni .prev, ni .new'
Check (-not (Test-Path "$($sb.Data)/update/resultat.json")) 'pas de resultat.json (la console tourne encore)'
Check ((Get-Content -Raw "$($sb.Dir)/kybernow.json" | ConvertFrom-Json).version -eq '0.2.5') 'kybernow.json reste en 0.2.5'
if ($global:listener) { $global:listener.Stop(); $global:listener = $null }

Write-Host '6. Station installee avant min_installer_version : rien n''est arrete'
$sb = New-Sandbox 'nouvelle console' -MinInstaller '0.2.6'
$err = Invoke-Update $sb
Check ($err -match 'rien n''a ete remplace' -and $err -match 'au moins la 0\.2\.6' -and $err -match 'installee : 0\.2\.5') "erreur annoncee ($err)"
Check ($global:svc['kyber'] -eq 'Running' -and $global:svc['kyber-admin'] -eq 'Running') 'services jamais arretes'
Check ((Read-File "$($sb.Dir)/admin/kyber-admin.exe") -match 'ancienne') 'console intacte'
if ($global:listener) { $global:listener.Stop(); $global:listener = $null }

Write-Host ''
if ($failures) { Write-Host "$failures echec(s)" -ForegroundColor Red; exit 1 } else { Write-Host 'Tout est bon.' -ForegroundColor Green }
