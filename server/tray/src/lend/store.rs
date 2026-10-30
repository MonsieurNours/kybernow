//! Ce que l'agent de console garde sur le disque pendant un prêt
//! (`%LOCALAPPDATA%\KyberNow\lend\state.json`), pour qu'un agent tombé en
//! plein prêt soit réparé au prochain `--console` ou au démarrage de l'icône :
//! les fenêtres réduites (et leur place), Steam (s'il tournait, la connexion
//! automatique, les fichiers mis de côté). Et la mise de côté des fichiers de
//! Steam elle-même. Sans Windows, pour les essais sur le Mac.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// Suffixe des fichiers de Steam mis de côté (`loginusers.vdf.kybernow`).
pub const ASIDE: &str = "kybernow";

/// Une fenêtre de l'utilisateur réduite pendant le prêt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub hwnd: i64,
    /// Processus de la fenêtre : une fenêtre d'un autre processus qui
    /// aurait repris le même numéro n'est pas touchée.
    pub pid: u32,
    /// `WINDOWPLACEMENT` : flags, showCmd, ptMinPosition (x, y),
    /// ptMaxPosition (x, y), rcNormalPosition (gauche, haut, droite, bas).
    pub placement: [i32; 10],
}

/// Steam pendant le prêt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Steam {
    /// « Son compte Steam » : Steam a changé de compte.
    pub friend: bool,
    pub exe: String,
    /// Steam tournait avant le prêt : il est rouvert à la fin.
    pub was_running: bool,
    /// `AutoLoginUser` d'avant le prêt (`None` : la valeur n'existait pas).
    pub auto_login: Option<String>,
    /// Fichiers mis de côté, et s'ils existaient avant le prêt.
    pub files: Vec<(PathBuf, bool)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    pub lend: String,
    /// Début du prêt (secondes Unix) : les processus nés après sont ceux du prêt.
    pub started: i64,
    /// Démarrage de Windows (secondes Unix, à quelques secondes près) : après
    /// un redémarrage, les numéros de fenêtres ne valent plus rien.
    pub boot: i64,
    pub windows: Vec<Window>,
    pub steam: Option<Steam>,
}

impl Saved {
    pub fn to_json(&self) -> Value {
        json!({
            "lend": self.lend,
            "started": self.started,
            "boot": self.boot,
            "windows": self.windows.iter().map(|w| json!({"hwnd": w.hwnd, "pid": w.pid, "placement": w.placement.to_vec()})).collect::<Vec<_>>(),
            "steam": self.steam.as_ref().map(|s| json!({
                "friend": s.friend,
                "exe": s.exe,
                "was_running": s.was_running,
                "auto_login": s.auto_login,
                "files": s.files.iter().map(|(p, existed)| json!({"path": p.to_string_lossy(), "existed": existed})).collect::<Vec<_>>(),
            })),
        })
    }

    pub fn from_json(v: &Value) -> Option<Saved> {
        let windows = v["windows"]
            .as_array()?
            .iter()
            .filter_map(|w| {
                let numbers: Vec<i32> = w["placement"].as_array()?.iter().filter_map(|n| i32::try_from(n.as_i64()?).ok()).collect();
                Some(Window { hwnd: w["hwnd"].as_i64()?, pid: u32::try_from(w["pid"].as_u64()?).ok()?, placement: numbers.try_into().ok()? })
            })
            .collect();
        let steam = v["steam"].as_object().map(|s| Steam {
            friend: s.get("friend").and_then(Value::as_bool).unwrap_or(false),
            exe: s.get("exe").and_then(Value::as_str).unwrap_or_default().to_string(),
            was_running: s.get("was_running").and_then(Value::as_bool).unwrap_or(false),
            auto_login: s.get("auto_login").and_then(Value::as_str).map(str::to_owned),
            files: s
                .get("files")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|f| Some((PathBuf::from(f["path"].as_str()?), f["existed"].as_bool()?)))
                .collect(),
        });
        Some(Saved {
            lend: v["lend"].as_str()?.to_string(),
            started: v["started"].as_i64()?,
            boot: v["boot"].as_i64().unwrap_or(0),
            windows,
            steam,
        })
    }
}

pub fn load(path: &Path) -> Option<Saved> {
    Saved::from_json(&serde_json::from_slice(&std::fs::read(path).ok()?).ok()?)
}

/// Écrit l'état d'un coup (fichier à côté, puis renommé).
pub fn save(path: &Path, saved: &Saved) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, saved.to_json().to_string())?;
    std::fs::rename(tmp, path)
}

pub fn clear(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// `loginusers.vdf` → `loginusers.vdf.kybernow`.
pub fn aside(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".");
    name.push(ASIDE);
    PathBuf::from(name)
}

/// Les fichiers de Steam qui retiennent les comptes : `config\loginusers.vdf`
/// (les comptes proposés), `config\config.vdf` (leurs jetons de connexion),
/// et `%LOCALAPPDATA%\Steam\local.vdf` (le jeton des Steam récents).
pub fn steam_files(steam_dir: &Path, local_app_data: Option<&Path>) -> Vec<PathBuf> {
    let config = steam_dir.join("config");
    let mut files = vec![config.join("loginusers.vdf"), config.join("config.vdf")];
    if let Some(local) = local_app_data {
        files.push(local.join("Steam").join("local.vdf"));
    }
    files
}

/// Chaque fichier, et s'il existe : noté avant de rien déplacer.
pub fn existing(files: &[PathBuf]) -> Vec<(PathBuf, bool)> {
    files.iter().map(|p| (p.clone(), p.is_file())).collect()
}

/// Met de côté les fichiers de l'utilisateur (déplacés, pas copiés : l'ami
/// ne doit pas voir les comptes retenus). Une copie de côté plus ancienne
/// (prêt mal fini) est remplacée : l'état sur le disque, écrit avant, dit
/// déjà ce qu'il faudra remettre.
pub fn put_aside(files: &[(PathBuf, bool)]) -> io::Result<()> {
    for (path, existed) in files {
        if *existed && path.is_file() {
            std::fs::rename(path, aside(path))?;
        }
    }
    Ok(())
}

/// Remet les fichiers de l'utilisateur, par-dessus ce que l'ami a laissé ; un
/// fichier qui n'existait pas avant le prêt est effacé (il tient les comptes
/// de l'ami). Va jusqu'au bout, et rend la première erreur.
pub fn put_back(files: &[(PathBuf, bool)]) -> io::Result<()> {
    let mut first = Ok(());
    for (path, existed) in files {
        let kept = aside(path);
        let result = if kept.is_file() {
            // `rename` remplace le fichier de l'ami (MoveFileEx, MOVEFILE_REPLACE_EXISTING).
            std::fs::rename(&kept, path)
        } else if !existed && path.exists() {
            std::fs::remove_file(path)
        } else {
            // Rien de côté : le fichier de l'utilisateur n'a pas bougé (agent
            // tombé avant de le déplacer).
            Ok(())
        };
        if first.is_ok() {
            first = result;
        }
    }
    first
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kybernow-lend-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("config")).unwrap();
        dir
    }

    #[test]
    fn etat_aller_retour() {
        let saved = Saved {
            lend: "l1".into(),
            started: 1_793_170_000,
            boot: 1_793_100_000,
            windows: vec![Window { hwnd: 0x1234, pid: 42, placement: [0, 3, -1, -1, -1, -1, 10, 20, 800, 600] }],
            steam: Some(Steam {
                friend: true,
                exe: r"C:\Steam\steam.exe".into(),
                was_running: true,
                auto_login: Some("matthieu".into()),
                files: vec![(PathBuf::from(r"C:\Steam\config\loginusers.vdf"), true), (PathBuf::from(r"C:\Steam\config\config.vdf"), false)],
            }),
        };
        assert_eq!(Saved::from_json(&saved.to_json()), Some(saved.clone()));
        let none = Saved { steam: None, windows: vec![], ..saved };
        assert_eq!(Saved::from_json(&none.to_json()), Some(none));
        let dir = temp("etat");
        let path = dir.join("lend").join("state.json");
        assert_eq!(load(&path), None);
        let s = Saved { lend: "l2".into(), started: 1, boot: 0, windows: vec![], steam: None };
        save(&path, &s).unwrap();
        assert_eq!(load(&path), Some(s));
        clear(&path);
        assert_eq!(load(&path), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn fichiers_de_steam() {
        let dir = temp("steam");
        let local = dir.join("local");
        let files = steam_files(&dir, Some(&local));
        assert_eq!(files[0], dir.join("config").join("loginusers.vdf"));
        assert_eq!(files[2], local.join("Steam").join("local.vdf"));
        assert_eq!(aside(&files[0]), dir.join("config").join("loginusers.vdf.kybernow"));
        // L'utilisateur a loginusers.vdf, pas de config.vdf ni de local.vdf.
        std::fs::write(&files[0], "comptes de l'utilisateur").unwrap();
        let noted = existing(&files);
        assert_eq!(noted.iter().map(|(_, e)| *e).collect::<Vec<_>>(), [true, false, false]);
        put_aside(&noted).unwrap();
        assert!(!files[0].exists());
        assert_eq!(std::fs::read_to_string(aside(&files[0])).unwrap(), "comptes de l'utilisateur");
        // L'ami se connecte : Steam écrit ses fichiers.
        std::fs::write(&files[0], "comptes de l'ami").unwrap();
        std::fs::write(&files[1], "jetons de l'ami").unwrap();
        put_back(&noted).unwrap();
        assert_eq!(std::fs::read_to_string(&files[0]).unwrap(), "comptes de l'utilisateur");
        assert!(!files[1].exists());
        assert!(!aside(&files[0]).exists());
        // Remise en état refaite (agent relancé) : rien ne bouge.
        put_back(&noted).unwrap();
        assert_eq!(std::fs::read_to_string(&files[0]).unwrap(), "comptes de l'utilisateur");
        // Agent tombé après avoir noté l'état, avant de déplacer : le fichier reste.
        put_back(&existing(&files)).unwrap();
        assert_eq!(std::fs::read_to_string(&files[0]).unwrap(), "comptes de l'utilisateur");
        let _ = std::fs::remove_dir_all(dir);
    }
}
