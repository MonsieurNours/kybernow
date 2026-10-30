//! Prêt de l'ordinateur : la dernière partie de chaque ami, que le lanceur
//! met en avant (« Dernière partie », docs/pret.md). Gardée par compte d'ami,
//! ou par appareil pour un code (`lend-last.json`) ; oubliée avec le compte.
//! Seule une app encore permise, ou un jeu Steam encore installé avec Steam
//! permis, est proposée : sinon, pas de grande tuile.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::lend::Who;

pub const FILE: &str = "lend-last.json";
/// Au plus ce nombre d'amis et d'appareils retenus (les plus anciens partent).
const MAX: usize = 200;

/// Ce qu'un ami a joué en dernier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Played {
    /// Une app permise, lancée depuis le lanceur (son identifiant de tuile).
    App { id: String },
    /// Un jeu Steam joué pendant le prêt.
    Steam { appid: u32 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Entry {
    played: Played,
    at: u64,
}

pub struct LastPlayed {
    path: PathBuf,
    map: Mutex<HashMap<String, Entry>>,
}

impl LastPlayed {
    pub fn load(data_dir: &Path) -> LastPlayed {
        let path = data_dir.join(FILE);
        let map = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        LastPlayed { path, map: Mutex::new(map) }
    }

    pub fn get(&self, key: &str) -> Option<Played> {
        self.map.lock().unwrap().get(key).map(|e| e.played.clone())
    }

    pub fn set(&self, key: &str, played: Played) {
        let mut map = self.map.lock().unwrap();
        if map.get(key).is_some_and(|e| e.played == played) {
            return;
        }
        map.insert(key.to_string(), Entry { played, at: now() });
        while map.len() > MAX {
            let Some(oldest) = map.iter().min_by_key(|(_, e)| e.at).map(|(k, _)| k.clone()) else { break };
            map.remove(&oldest);
        }
        self.save(&map);
    }

    pub fn forget(&self, key: &str) {
        let mut map = self.map.lock().unwrap();
        if map.remove(key).is_some() {
            self.save(&map);
        }
    }

    fn save(&self, map: &HashMap<String, Entry>) {
        let tmp = self.path.with_extension("json.tmp");
        let written = serde_json::to_vec_pretty(map).map_err(std::io::Error::other).and_then(|b| std::fs::write(&tmp, b)).and_then(|_| std::fs::rename(&tmp, &self.path));
        if let Err(e) = written {
            tracing::warn!("prêt : dernière partie non gardée : {e}");
        }
    }
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// La clé d'un ami : son compte ; venu par un code, son appareil.
pub fn key_of(who: &Who) -> Option<String> {
    match (&who.friend, who.device.trim()) {
        (Some(friend), _) => Some(friend_key(friend)),
        (None, "") => None,
        (None, device) => Some(format!("device:{device}")),
    }
}

pub fn friend_key(friend: &str) -> String {
    format!("friend:{friend}")
}

/// Un jeu Steam installé : son nom (son `appmanifest`) et son image, si le
/// cache de la bibliothèque de Steam en a une.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SteamGame {
    pub name: String,
    pub image: Option<PathBuf>,
}

/// La tuile « Dernière partie » pour l'agent de console, ou rien : une app
/// qui n'est plus permise (la tuile Steam elle-même n'en est pas une), un
/// jeu Steam désinstallé ou Steam plus permis. `apps` : les tuiles du lanceur.
pub fn tile(played: &Played, apps: &[Value], steam_allowed: bool, steam_game: impl Fn(u32) -> Option<SteamGame>) -> Option<Value> {
    match played {
        Played::App { id } => {
            let app = apps.iter().find(|a| a["id"] == id.as_str() && id != "steam")?;
            Some(json!({ "id": id, "kind": "app", "name": app["name"], "icon": app["icon"], "image": null }))
        }
        Played::Steam { appid } => {
            if !steam_allowed {
                return None;
            }
            let game = steam_game(*appid)?;
            Some(json!({
                "id": format!("steam:{appid}"),
                "kind": "steam",
                "name": game.name,
                "icon": null,
                "image": game.image.map(|_| format!("/api/local/lend/icons/steam-{appid}.jpg")),
            }))
        }
    }
}

/// `steam:<appid>` d'une tuile « Dernière partie ».
pub fn steam_appid(id: &str) -> Option<u32> {
    id.strip_prefix("steam:")?.parse().ok().filter(|n| *n > 0)
}

/// `steam-<appid>.jpg`, l'image d'un jeu Steam servie à l'agent.
pub fn steam_image_appid(file: &str) -> Option<u32> {
    file.strip_prefix("steam-")?.strip_suffix(".jpg")?.parse().ok().filter(|n| *n > 0)
}

// --- Les fichiers de Steam ------------------------------------------------------------------

/// Les valeurs d'une clé dans un fichier VDF de Steam (`"clé"  "valeur"`),
/// sans échappements (`\\` → `\`).
fn vdf_values(text: &str, key: &str) -> Vec<String> {
    let quoted = format!("\"{key}\"");
    text.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix(&quoted)?.trim();
            let value = rest.strip_prefix('"')?.strip_suffix('"')?;
            Some(value.replace("\\\\", "\\"))
        })
        .collect()
}

/// Les bibliothèques de Steam : son dossier, puis celles de `libraryfolders.vdf`.
fn libraries(steam: &Path) -> Vec<PathBuf> {
    let mut libs = vec![steam.to_path_buf()];
    if let Ok(text) = std::fs::read_to_string(steam.join("steamapps").join("libraryfolders.vdf")) {
        for path in vdf_values(&text, "path") {
            let path = PathBuf::from(path);
            if !libs.contains(&path) {
                libs.push(path);
            }
        }
    }
    libs
}

/// Le jeu `appid`, s'il est installé dans une bibliothèque de Steam.
pub fn steam_game(steam: &Path, appid: u32) -> Option<SteamGame> {
    let manifest = libraries(steam).into_iter().map(|lib| lib.join("steamapps").join(format!("appmanifest_{appid}.acf"))).find(|p| p.is_file())?;
    let text = std::fs::read_to_string(manifest).ok()?;
    let name = vdf_values(&text, "name").into_iter().next().filter(|n| !n.trim().is_empty())?;
    Some(SteamGame { name, image: steam_image(steam, appid) })
}

/// L'image d'un jeu dans le cache de la bibliothèque de Steam : l'image large
/// (fond du lanceur, tuile en 16:9), sinon l'en-tête, sinon la jaquette.
/// Anciens noms (`<appid>_library_hero.jpg`) et nouveaux
/// (`<appid>/library_hero.jpg`, parfois un dossier plus bas).
fn steam_image(steam: &Path, appid: u32) -> Option<PathBuf> {
    const NAMES: [&str; 3] = ["library_hero.jpg", "header.jpg", "library_600x900.jpg"];
    let cache = steam.join("appcache").join("librarycache");
    let folder = cache.join(appid.to_string());
    let below: Vec<PathBuf> = std::fs::read_dir(&folder).map(|d| d.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default();
    NAMES.iter().find_map(|name| {
        let old = cache.join(format!("{appid}_{name}"));
        let new = folder.join(name);
        [old, new].into_iter().chain(below.iter().map(|d| d.join(name))).find(|p| p.is_file())
    })
}

/// Le dossier de Steam de l'utilisateur, relu toutes les minutes au plus.
pub fn steam_root() -> Option<PathBuf> {
    static CACHE: Mutex<Option<(Option<PathBuf>, Instant)>> = Mutex::new(None);
    if let Some((dir, at)) = CACHE.lock().unwrap().clone()
        && at.elapsed() < Duration::from_secs(60)
    {
        return dir;
    }
    let dir = crate::lend_sys::steam_dir();
    *CACHE.lock().unwrap() = Some((dir.clone(), Instant::now()));
    dir
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lend::Kind;

    fn apps() -> Vec<Value> {
        vec![
            json!({ "id": "steam", "name": "Steam", "icon": null }),
            json!({ "id": "a1b2c3d4e5f60718", "name": "Celeste", "icon": "/api/local/lend/icons/a1b2c3d4e5f60718.png" }),
        ]
    }

    fn hades(appid: u32) -> Option<SteamGame> {
        (appid == 1145360).then(|| SteamGame { name: "Hades".into(), image: Some(PathBuf::from("x.jpg")) })
    }

    #[test]
    fn la_derniere_partie_seulement_si_elle_est_encore_permise() {
        // Une app permise : sa tuile, son icône.
        let tile = tile(&Played::App { id: "a1b2c3d4e5f60718".into() }, &apps(), true, hades).unwrap();
        assert_eq!((tile["id"].as_str(), tile["kind"].as_str(), tile["name"].as_str()), (Some("a1b2c3d4e5f60718"), Some("app"), Some("Celeste")));
        assert_eq!(tile["icon"], "/api/local/lend/icons/a1b2c3d4e5f60718.png");
        // Retirée des apps permises, ou la tuile Steam elle-même : rien.
        assert!(super::tile(&Played::App { id: "0000000000000000".into() }, &apps(), true, hades).is_none());
        assert!(super::tile(&Played::App { id: "steam".into() }, &apps(), true, hades).is_none());
        // Un jeu Steam installé, Steam permis : son nom et son image.
        let tile = super::tile(&Played::Steam { appid: 1145360 }, &apps(), true, hades).unwrap();
        assert_eq!((tile["id"].as_str(), tile["name"].as_str()), (Some("steam:1145360"), Some("Hades")));
        assert_eq!(tile["image"], "/api/local/lend/icons/steam-1145360.jpg");
        // Steam plus permis, ou le jeu désinstallé : rien.
        assert!(super::tile(&Played::Steam { appid: 1145360 }, &apps(), false, hades).is_none());
        assert!(super::tile(&Played::Steam { appid: 620 }, &apps(), true, hades).is_none());
        // Sans image dans le cache de Steam : la tuile sans image (le pictogramme).
        let tile = super::tile(&Played::Steam { appid: 1145360 }, &apps(), true, |_| Some(SteamGame { name: "Hades".into(), image: None })).unwrap();
        assert!(tile["image"].is_null());
    }

    #[test]
    fn une_partie_par_ami_ou_par_appareil() {
        let dir = std::env::temp_dir().join(format!("kn-lend-last-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut who = Who::new("Léa", "ipad-1", "ios", false, Kind::Code, None);
        assert_eq!(key_of(&who).as_deref(), Some("device:ipad-1"));
        who.friend = Some("f1".into());
        assert_eq!(key_of(&who).as_deref(), Some("friend:f1"));
        assert_eq!(key_of(&Who::new("Léa", "", "web", false, Kind::Code, None)), None);

        let last = LastPlayed::load(&dir);
        last.set("friend:f1", Played::Steam { appid: 1145360 });
        last.set("device:ipad-1", Played::App { id: "a1".into() });
        // Gardé sur disque, relu au démarrage.
        let again = LastPlayed::load(&dir);
        assert_eq!(again.get("friend:f1"), Some(Played::Steam { appid: 1145360 }));
        assert_eq!(again.get("device:ipad-1"), Some(Played::App { id: "a1".into() }));
        // Oublié avec le compte.
        again.forget("friend:f1");
        assert_eq!(LastPlayed::load(&dir).get("friend:f1"), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn les_jeux_installes_de_steam() {
        let dir = std::env::temp_dir().join(format!("kn-steam-{}", rand::random::<u64>()));
        let (steam, lib) = (dir.join("Steam"), dir.join("SteamLibrary"));
        std::fs::create_dir_all(steam.join("steamapps")).unwrap();
        std::fs::create_dir_all(lib.join("steamapps")).unwrap();
        let lib_vdf = lib.display().to_string().replace('\\', "\\\\");
        std::fs::write(steam.join("steamapps/libraryfolders.vdf"), format!("\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{lib_vdf}\"\n\t}}\n}}\n")).unwrap();
        std::fs::write(lib.join("steamapps/appmanifest_1145360.acf"), "\"AppState\"\n{\n\t\"appid\"\t\t\"1145360\"\n\t\"name\"\t\t\"Hades\"\n}\n").unwrap();
        // Sans image : le jeu, sans image.
        assert_eq!(steam_game(&steam, 1145360), Some(SteamGame { name: "Hades".into(), image: None }));
        // Une image au nouveau format, dans un sous-dossier.
        let cover = steam.join("appcache/librarycache/1145360/abc123/library_600x900.jpg");
        std::fs::create_dir_all(cover.parent().unwrap()).unwrap();
        std::fs::write(&cover, b"jpg").unwrap();
        assert_eq!(steam_game(&steam, 1145360).unwrap().image, Some(cover));
        // L'image large passe avant (ancien format, à plat).
        let hero = steam.join("appcache/librarycache/1145360_library_hero.jpg");
        std::fs::write(&hero, b"jpg").unwrap();
        assert_eq!(steam_game(&steam, 1145360).unwrap().image, Some(hero));
        // Pas installé.
        assert_eq!(steam_game(&steam, 620), None);
        assert_eq!((steam_appid("steam:1145360"), steam_appid("steam:x"), steam_image_appid("steam-620.jpg"), steam_image_appid("../a.jpg")), (Some(1145360), None, Some(620), None));
        let _ = std::fs::remove_dir_all(dir);
    }
}
