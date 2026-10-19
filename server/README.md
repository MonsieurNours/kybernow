# kyber-admin

Interface web d'administration d'un serveur Kyber sous Windows : réglages de `kyber_config.toml` (commentaires préservés, sauvegarde à chaque écriture), utilisateurs Kyber, démarrage et arrêt du service `kyber`, état du GPU NVIDIA (NVML) et journaux.

- `admin/` : service Rust (axum), HTTPS sur le port **9443** avec un certificat auto-signé généré au premier lancement.
- `web/` : interface Svelte, compilée puis embarquée dans le binaire.

## Développer sur le Mac

```bash
cd server/web && npm run build                       # ou npm run dev (proxy /api vers 9443)
cd ../admin && cargo run -- --http --listen 127.0.0.1:9443 \
    --data-dir dev-data --kyber-dir dev-data/kyber    # copier un kyber_config.toml dans dev-data/kyber
```

Hors Windows, le service Kyber est simulé et il n'y a pas de supervision GPU.

## Compiler pour Windows

```bash
server/build-windows.sh        # → server/dist/kyber-admin.exe
```

## Installer sur le PC (PowerShell administrateur)

```powershell
New-Item -ItemType Directory -Force "C:\Program Files\KyberAdmin" | Out-Null
Move-Item "$env:USERPROFILE\Downloads\kyber-admin.exe" "C:\Program Files\KyberAdmin\" -Force
& "C:\Program Files\KyberAdmin\kyber-admin.exe" service install
New-NetFirewallRule -DisplayName "Kyber Admin (TCP 9443)" -Direction Inbound -Protocol TCP -LocalPort 9443 -Action Allow -Profile Private
Get-Content C:\ProgramData\KyberAdmin\setup-code.txt
```

Puis ouvrir `https://<IP-du-PC>:9443`, accepter le certificat, saisir le code d'initialisation et choisir le mot de passe administrateur.

| Emplacement | Contenu |
|---|---|
| `C:\ProgramData\KyberAdmin\kyber-admin.toml` | réglages : `listen`, `kyber_dir`, empreinte du mot de passe |
| `C:\ProgramData\KyberAdmin\tls\` | certificat de l'interface |
| `C:\ProgramData\KyberAdmin\backups\` | 30 dernières versions de `kyber_config.toml` |
| `C:\ProgramData\KyberAdmin\kyber-admin.log` | journal du service |

Autres commandes : `kyber-admin.exe service uninstall`, `kyber-admin.exe reset-password` (puis redémarrer le service), `kyber-admin.exe` seul pour lancer en console (diagnostic).
