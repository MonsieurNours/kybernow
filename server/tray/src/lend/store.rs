//! Ce que l'agent de console garde sur le disque pendant un prêt
//! (`%LOCALAPPDATA%\KyberNow\lend\state.json`), pour qu'un agent tombé en
//! plein prêt soit réparé au prochain `--console` ou au démarrage de l'icône :
//! les fenêtres réduites (et leur place), Steam (s'il tournait, la connexion
//! automatique, les fichiers mis de côté). Et la mise de côté des fichiers de
//! Steam elle-même : sous Windows, leur contenu est confié à Admin (SYSTEM,
//! [`Keeper`]) et les originaux effacés, pour que l'ami, qui joue avec le
//! même compte, ne puisse pas les lire (STWIN-14) ; sur Mac et sous Linux,
//! Admin tourne avec ce même compte : ils sont renommés à côté. Sans
//! Windows, pour les essais sur le Mac.

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
    /// Leur contenu est gardé par Admin (Windows), pas renommé à côté.
    pub held: bool,
}

/// Admin qui garde le contenu des fichiers de Steam de l'hôte pendant le
/// prêt (Windows : `/api/local/lend/console/steam/*`).
pub trait Keeper {
    /// Les confie à Admin (sur son disque avant la réponse).
    fn keep(&self, lend: &str, files: &[(String, Vec<u8>)]) -> anyhow::Result<()>;
    /// Ce qu'Admin garde pour ce prêt ; `Ok(None)` : rien (déjà remis).
    fn back(&self, lend: &str) -> anyhow::Result<Option<Vec<(String, Vec<u8>)>>>;
    /// Remis : Admin peut les oublier.
    fn restored(&self, lend: &str);
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
                "held": s.held,
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
            // Un état d'avant la 0.11.8 : fichiers renommés à côté.
            held: s.get("held").and_then(Value::as_bool).unwrap_or(false),
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

/// Mac et Linux : met de côté les fichiers de l'utilisateur (déplacés, pas
/// copiés : Steam de l'ami ne doit pas voir les comptes retenus). Jamais par-dessus une copie de côté
/// déjà là (un prêt d'avant pas remis en état : ce sont peut-être les seuls
/// fichiers de l'utilisateur) : rien ne bouge, erreur.
#[cfg_attr(windows, allow(dead_code))]
pub fn put_aside(files: &[(PathBuf, bool)]) -> io::Result<()> {
    if let Some((path, _)) = files.iter().find(|(path, _)| aside(path).exists()) {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} existe déjà", aside(path).display())));
    }
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

/// Le nom d'un fichier de Steam, tel qu'Admin le garde (`config.vdf`).
pub fn name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Le contenu des fichiers de l'utilisateur qui existent, à confier à Admin.
pub fn contents(files: &[(PathBuf, bool)]) -> io::Result<Vec<(String, Vec<u8>)>> {
    files.iter().filter(|(path, existed)| *existed && path.is_file()).map(|(path, _)| Ok((name(path), std::fs::read(path)?))).collect()
}

/// Efface les fichiers listés (après les avoir confiés à Admin ; ou, à la
/// fin, ceux de l'ami quand Admin ne répond pas). Va jusqu'au bout, et rend
/// la première erreur.
pub fn remove(files: &[(PathBuf, bool)]) -> io::Result<()> {
    let mut first = Ok(());
    for (path, _) in files {
        if path.exists()
            && let Err(e) = std::fs::remove_file(path)
            && first.is_ok()
        {
            first = Err(e);
        }
    }
    first
}

/// Réécrit les fichiers de l'utilisateur rendus par Admin, par-dessus ce que
/// l'ami a laissé (fichier écrit à côté, puis renommé) ; un fichier qui
/// n'existait pas avant le prêt est effacé (il tient les comptes de l'ami).
pub fn write_back(files: &[(PathBuf, bool)], kept: &[(String, Vec<u8>)]) -> io::Result<()> {
    let mut first = Ok(());
    for (path, existed) in files {
        let result = match kept.iter().find(|(n, _)| *n == name(path)) {
            Some((_, data)) => {
                let tmp = aside(path);
                std::fs::write(&tmp, data).and_then(|()| std::fs::rename(&tmp, path))
            }
            None if !existed && path.exists() => std::fs::remove_file(path),
            None => Ok(()),
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
                held: true,
            }),
        };
        assert_eq!(Saved::from_json(&saved.to_json()), Some(saved.clone()));
        // L'état d'un agent d'avant la 0.11.8 : fichiers renommés à côté.
        let mut old = saved.to_json();
        old["steam"].as_object_mut().unwrap().remove("held");
        assert!(!Saved::from_json(&old).unwrap().steam.unwrap().held);
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
        // Une copie de côté déjà là (prêt d'avant pas remis) : rien ne bouge.
        std::fs::write(aside(&files[0]), "seule copie de l'utilisateur").unwrap();
        std::fs::write(&files[0], "comptes de l'ami d'avant").unwrap();
        assert_eq!(put_aside(&existing(&files)).unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(aside(&files[0])).unwrap(), "seule copie de l'utilisateur");
        assert_eq!(std::fs::read_to_string(&files[0]).unwrap(), "comptes de l'ami d'avant");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Windows (STWIN-14) : le contenu confié à Admin, rien laissé à côté.
    #[test]
    fn fichiers_de_steam_gardes_par_admin() {
        let dir = temp("garde");
        let files = steam_files(&dir, Some(&dir.join("local")));
        std::fs::create_dir_all(dir.join("local").join("Steam")).unwrap();
        std::fs::write(&files[0], "comptes de l'utilisateur").unwrap();
        std::fs::write(&files[2], "jeton de l'utilisateur").unwrap();
        let noted = existing(&files);
        let kept = contents(&noted).unwrap();
        assert_eq!(kept, [("loginusers.vdf".to_string(), b"comptes de l'utilisateur".to_vec()), ("local.vdf".to_string(), b"jeton de l'utilisateur".to_vec())]);
        remove(&noted).unwrap();
        assert!(files.iter().all(|f| !f.exists() && !aside(f).exists()), "rien sur le disque pendant le prêt");
        // L'ami se connecte : Steam écrit ses fichiers, dont config.vdf.
        for f in &files {
            std::fs::write(f, "de l'ami").unwrap();
        }
        write_back(&noted, &kept).unwrap();
        assert_eq!(std::fs::read_to_string(&files[0]).unwrap(), "comptes de l'utilisateur");
        assert!(!files[1].exists(), "config.vdf n'existait pas avant le prêt");
        assert_eq!(std::fs::read_to_string(&files[2]).unwrap(), "jeton de l'utilisateur");
        assert!(files.iter().all(|f| !aside(f).exists()));
        // Admin injoignable à la fin : les fichiers de l'ami partent quand même.
        for f in &files {
            std::fs::write(f, "de l'ami").unwrap();
        }
        remove(&noted).unwrap();
        assert!(files.iter().all(|f| !f.exists()));
        let _ = std::fs::remove_dir_all(dir);
    }
}
