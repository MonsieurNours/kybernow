//! Consoles PlayStation de l'app (docs/playstation.md ; textes :
//! docs/link-ui.md, « Consoles PlayStation ») sans rien de Windows : les
//! comptes PlayStation Network et les consoles associées
//! (`%APPDATA%\KyberNow\playstation.json`), leur état vu d'ici, ce que
//! l'interface en reçoit, et le paquet de « Envoyer à un appareil »
//! (format commun à toutes les apps : note `format-playstation-icloud-et-envoi`
//! de la session Mac, recopiée dans docs/playstation.md « Plusieurs
//! appareils »).
//!
//! Les secrets (l'identifiant de chaque compte, et chaque association, qui
//! porte les clés de la console) ne sont jamais dans le fichier : ils vont au
//! gestionnaire d'identifiants de Windows ([`Secrets`] ; `KyberNow:ps-account-<id>`
//! et `KyberNow:ps-<HOSTID>.<id>`). Le fichier garde, par console, la liste
//! des comptes associés, relue au lancement contre le gestionnaire.
//!
//! Testé sur le Mac (`cargo test -p kybernow-windows`), avec des secrets en mémoire.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use kybernow_core::playstation::keymouse::Layout;
use kybernow_core::playstation::settings::PlaySettings;

/// Sans réponse depuis ce délai : la console est hors ligne (comme sur Mac).
pub const OFFLINE_AFTER: Duration = Duration::from_secs(12);

/// Les secrets des consoles : le gestionnaire d'identifiants sous Windows,
/// une table en mémoire dans les tests.
pub trait Secrets {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&mut self, key: &str, value: &str) -> bool;
    fn delete(&mut self, key: &str);
}

/// Secret de l'identifiant d'un compte.
pub fn account_secret(account: &str) -> String {
    format!("ps-account-{account}")
}

/// Secret d'une association (console, compte).
pub fn pair_secret(host_id: &str, account: &str) -> String {
    format!("ps-{}.{account}", host_id.to_ascii_uppercase())
}

/// Un identifiant de compte tiré au hasard, comme le `uuidString` de Swift
/// (UUID v4 en MAJUSCULES) : le même format sur tous les appareils.
pub fn new_uuid() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::getrandom(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02X}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// Un identifiant PSN saisi à la main : 8 octets en base64 (12 caractères).
pub fn valid_psn_id(text: &str) -> Option<String> {
    use base64::Engine;
    let t = text.trim();
    let bytes = base64::engine::general_purpose::STANDARD.decode(t).ok()?;
    (bytes.len() == 8).then(|| t.to_string())
}

/// Un compte PlayStation Network gardé sur ce PC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
    /// UUID en majuscules (le même sur tous les appareils qui l'ont reçu).
    pub id: String,
    /// Pseudo PSN lu chez Sony, ou nom donné à la main ; `None` : compte
    /// d'une version précédente (« Compte PlayStation »).
    #[serde(default)]
    pub name: Option<String>,
}

/// Une console gardée (associée au moins une fois).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    /// Identifiant de la console (`mac` de l'association), en MAJUSCULES.
    pub host_id: String,
    pub name: String,
    /// « PS5 » ou « PS4 ».
    pub model: String,
    /// Dernière adresse connue (la recherche la tient à jour).
    pub address: String,
    /// La cible de l'association (`ps5_v1`…), pour associer de nouveau.
    #[serde(default)]
    pub target: String,
    /// Les comptes associés (leur association est au gestionnaire).
    #[serde(default)]
    pub accounts: Vec<String>,
    /// Le compte choisi à l'accueil (puce « {pseudo} »).
    #[serde(default)]
    pub account: Option<String>,
    /// Réglages (JSON du cœur) ; absent : Auto.
    #[serde(default)]
    pub settings: Option<PlaySettings>,
    /// Disposition du clavier et de la souris ; absente : par défaut.
    #[serde(default)]
    pub layout: Option<Layout>,
}

impl Kept {
    pub fn settings(&self) -> PlaySettings {
        self.settings.clone().unwrap_or_default()
    }

    pub fn layout(&self) -> Layout {
        self.layout.clone().unwrap_or_default()
    }

    pub fn is_ps5(&self) -> bool {
        self.model.eq_ignore_ascii_case("PS5")
    }

    /// La cible à associer (gardée, sinon celle de son modèle).
    pub fn target_or_default(&self) -> String {
        if self.target.is_empty() {
            if self.is_ps5() { "ps5_v1".into() } else { "ps4_v10".into() }
        } else {
            self.target.clone()
        }
    }
}

/// Ce que `playstation.json` garde.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PsFile {
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub consoles: Vec<Kept>,
    /// Adresses saisies dans « Par adresse » où une console a répondu :
    /// cherchées en direct, en plus du réseau local.
    #[serde(default)]
    pub addresses: Vec<String>,
    /// Les Stations (ordinateurs de l'app) qui voient chaque console, par
    /// identifiant de console (appris sur le réseau, `GET /api/consoles`).
    #[serde(default)]
    pub seen_by: BTreeMap<String, Vec<String>>,
}

/// Une console qui répond (découverte, adresse retenue).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Found {
    pub host_id: String,
    pub name: String,
    pub model: String,
    pub address: String,
    /// `ready`, `standby`, `unknown`.
    pub state: String,
    pub target: String,
    pub running_app: Option<String>,
}

impl Found {
    pub fn ready(&self) -> bool {
        self.state == "ready"
    }
}

/// L'état d'une console vu d'ici.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Searching,
    On,
    Standby,
    Offline,
    /// Elle ne répond pas sur le réseau, mais une Station qui la voit est
    /// joignable par le relais (« Hors de chez vous »).
    Away,
}

/// Un compte dans les listes de l'interface.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccountView {
    pub id: String,
    pub name: Option<String>,
    pub consoles: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccountRef {
    pub id: String,
    pub name: Option<String>,
}

/// Une console gardée pour l'interface (`PsConsole` de `ui/src/lib/api.ts`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConsoleView {
    pub id: String,
    pub host_id: String,
    pub name: String,
    pub model: String,
    pub address: String,
    pub status: Status,
    pub running_app: Option<String>,
    pub accounts: Vec<AccountRef>,
    pub chosen: Option<String>,
    pub free_accounts: usize,
    pub custom: bool,
    pub waking: bool,
    pub reaching: bool,
    pub in_session: bool,
    pub found: Option<Found>,
}

/// Tout l'état des consoles pour l'interface (`PsState`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StateView {
    pub accounts: Vec<AccountView>,
    pub consoles: Vec<ConsoleView>,
    pub found: Vec<Found>,
    pub searched: bool,
}

/// L'identifiant d'une console dans l'app (sessions, sélection) : `ps-<HOSTID>`.
pub fn console_id(host_id: &str) -> String {
    format!("ps-{}", host_id.to_ascii_uppercase())
}

/// L'identifiant de console d'un identifiant de l'app (`ps-<HOSTID>`).
pub fn host_of(id: &str) -> Option<&str> {
    id.strip_prefix("ps-")
}

/// Les consoles de l'app et ce qui en est vu en ce moment.
#[derive(Debug, Default)]
pub struct Consoles {
    pub file: PsFile,
    /// Dernière réponse de chaque console, par identifiant.
    pub found: HashMap<String, (Instant, Found)>,
    /// Une recherche au moins a abouti.
    pub searched: bool,
    /// Réveil en cours, et hors de chez soi : la Station est jointe.
    pub waking: Vec<String>,
    pub reaching: Vec<String>,
}

impl Consoles {
    pub fn load(path: &Path) -> Self {
        let file = std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        Consoles { file, ..Default::default() }
    }

    pub fn save(&self, path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let Ok(json) = serde_json::to_vec_pretty(&self.file) else { return };
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, &json).is_ok() && std::fs::rename(&tmp, path).is_err() {
            let _ = std::fs::write(path, &json);
        }
    }

    /// Au lancement : les associations dont le secret a disparu (gestionnaire
    /// vidé) ne comptent plus ; les comptes sans identifiant non plus.
    pub fn check_secrets(&mut self, secrets: &dyn Secrets) -> bool {
        let before = self.file.clone();
        self.file.accounts.retain(|a| secrets.get(&account_secret(&a.id)).is_some());
        let accounts: Vec<String> = self.file.accounts.iter().map(|a| a.id.clone()).collect();
        for c in &mut self.file.consoles {
            let host = c.host_id.clone();
            c.accounts.retain(|a| secrets.get(&pair_secret(&host, a)).is_some());
            if c.account.as_ref().is_some_and(|a| !accounts.contains(a)) {
                c.account = None;
            }
        }
        before != self.file
    }

    pub fn kept(&self, host_id: &str) -> Option<&Kept> {
        self.file.consoles.iter().find(|c| c.host_id.eq_ignore_ascii_case(host_id))
    }

    pub fn kept_mut(&mut self, host_id: &str) -> Option<&mut Kept> {
        self.file.consoles.iter_mut().find(|c| c.host_id.eq_ignore_ascii_case(host_id))
    }

    /// Sa dernière réponse, si elle date de moins de 12 s.
    pub fn live(&self, host_id: &str, now: Instant) -> Option<&Found> {
        self.found
            .get(&host_id.to_ascii_uppercase())
            .filter(|(at, _)| now.saturating_duration_since(*at) < OFFLINE_AFTER)
            .map(|(_, f)| f)
    }

    /// `away_station` : une Station qui la voit est joignable par le relais.
    pub fn status(&self, c: &Kept, now: Instant, away_station: bool) -> Status {
        if let Some(f) = self.live(&c.host_id, now) {
            return if f.ready() { Status::On } else { Status::Standby };
        }
        if away_station {
            return Status::Away;
        }
        if self.searched { Status::Offline } else { Status::Searching }
    }

    /// Une recherche a répondu : retenu, l'adresse ou le nom changés aussi.
    /// Vrai si une console gardée a changé.
    pub fn record(&mut self, list: Vec<Found>, now: Instant) -> bool {
        let mut changed = false;
        for f in list {
            if let Some(c) = self.kept_mut(&f.host_id) {
                if c.address != f.address || c.name != f.name {
                    c.address = f.address.clone();
                    c.name = f.name.clone();
                    changed = true;
                }
            }
            self.found.insert(f.host_id.to_ascii_uppercase(), (now, f));
        }
        self.searched = true;
        changed
    }

    /// Les adresses à chercher en direct : celles des consoles gardées, puis
    /// celles de « Par adresse ».
    pub fn direct(&self) -> Vec<String> {
        let mut list: Vec<String> = self.file.consoles.iter().map(|c| c.address.clone()).filter(|a| !a.is_empty()).collect();
        for a in &self.file.addresses {
            if !list.contains(a) {
                list.push(a.clone());
            }
        }
        list
    }

    /// « Par adresse » : une console a répondu à cette adresse.
    pub fn remember(&mut self, address: &str, f: Found, now: Instant) {
        self.found.insert(f.host_id.to_ascii_uppercase(), (now, f));
        if !self.file.addresses.iter().any(|a| a == address) {
            self.file.addresses.push(address.to_string());
        }
    }

    /// Toutes les consoles qui répondent, par nom.
    pub fn on_network(&self, now: Instant) -> Vec<Found> {
        let mut v: Vec<Found> = self
            .found
            .values()
            .filter(|(at, _)| now.saturating_duration_since(*at) < OFFLINE_AFTER)
            .map(|(_, f)| f.clone())
            .collect();
        v.sort_by(|a, b| a.name.cmp(&b.name).then(a.host_id.cmp(&b.host_id)));
        v
    }

    // MARK: - Comptes

    /// Les consoles associées avec ce compte.
    pub fn consoles_of(&self, account: &str) -> usize {
        self.file.consoles.iter().filter(|c| c.accounts.iter().any(|a| a == account)).count()
    }

    /// Un compte lu chez Sony ou saisi à la main : gardé. Même identifiant
    /// qu'un compte gardé : son pseudo est mis à jour, sans doublon (vrai :
    /// il était déjà là).
    pub fn keep_account(&mut self, secrets: &mut dyn Secrets, psn_id: &str, name: Option<&str>) -> Option<(Account, bool)> {
        let name = name.map(str::trim).filter(|n| !n.is_empty()).map(str::to_string);
        if let Some(a) = self.file.accounts.iter_mut().find(|a| secrets.get(&account_secret(&a.id)).as_deref() == Some(psn_id)) {
            if name.is_some() && a.name != name {
                a.name = name;
            }
            return Some((a.clone(), true));
        }
        let a = Account { id: new_uuid(), name };
        if !secrets.set(&account_secret(&a.id), psn_id) {
            return None;
        }
        self.file.accounts.push(a.clone());
        Some((a, false))
    }

    pub fn rename_account(&mut self, account: &str, name: &str) -> bool {
        let name = name.trim();
        match self.file.accounts.iter_mut().find(|a| a.id == account) {
            Some(a) if !name.is_empty() => {
                a.name = Some(name.to_string());
                true
            }
            _ => false,
        }
    }

    /// « Retirer » : ses associations sont effacées ; une console qui n'en a
    /// plus d'autre est retirée, une console associée aussi avec un autre
    /// compte reste, avec cet autre compte.
    pub fn remove_account(&mut self, secrets: &mut dyn Secrets, account: &str) {
        let mut emptied = Vec::new();
        for c in &mut self.file.consoles {
            if c.accounts.iter().any(|a| a == account) {
                secrets.delete(&pair_secret(&c.host_id, account));
                c.accounts.retain(|a| a != account);
                if c.accounts.is_empty() {
                    emptied.push(c.host_id.clone());
                }
            }
            if c.account.as_deref() == Some(account) {
                c.account = None;
            }
        }
        self.file.consoles.retain(|c| !emptied.contains(&c.host_id));
        secrets.delete(&account_secret(account));
        self.file.accounts.retain(|a| a.id != account);
    }

    /// Les comptes gardés pas encore associés à cette console.
    pub fn free_accounts(&self, host_id: &str) -> Vec<&Account> {
        let paired = self.kept(host_id).map(|c| c.accounts.clone()).unwrap_or_default();
        self.file.accounts.iter().filter(|a| !paired.contains(&a.id)).collect()
    }

    /// Les comptes associés, dans l'ordre des comptes.
    pub fn paired_accounts(&self, c: &Kept) -> Vec<&Account> {
        self.file.accounts.iter().filter(|a| c.accounts.contains(&a.id)).collect()
    }

    /// Le compte avec lequel la console ouvre la partie : celui choisi, sinon
    /// le premier associé.
    pub fn chosen(&self, c: &Kept) -> Option<String> {
        let list = self.paired_accounts(c);
        list.iter().find(|a| Some(&a.id) == c.account.as_ref()).or(list.first()).map(|a| a.id.clone())
    }

    // MARK: - Associations

    /// Une console vient d'être associée avec ce compte : gardée (ou mise à
    /// jour), et ce compte choisi pour elle.
    pub fn paired(&mut self, secrets: &mut dyn Secrets, f: &Found, host_id: &str, registered: &str, account: &str) -> bool {
        let host_id = host_id.to_ascii_uppercase();
        if !secrets.set(&pair_secret(&host_id, account), registered) {
            return false;
        }
        match self.kept_mut(&host_id) {
            Some(c) => {
                c.name = f.name.clone();
                c.address = f.address.clone();
                if !f.target.is_empty() {
                    c.target = f.target.clone();
                }
                if !c.accounts.iter().any(|a| a == account) {
                    c.accounts.push(account.to_string());
                }
                c.account = Some(account.to_string());
            }
            None => self.file.consoles.push(Kept {
                host_id: host_id.clone(),
                name: f.name.clone(),
                model: f.model.clone(),
                address: f.address.clone(),
                target: f.target.clone(),
                accounts: vec![account.to_string()],
                account: Some(account.to_string()),
                settings: None,
                layout: None,
            }),
        }
        true
    }

    /// La console ne reconnaît plus ce compte : son association est effacée
    /// (« Associer » revient si c'était la dernière ; la console reste).
    pub fn unpair(&mut self, secrets: &mut dyn Secrets, host_id: &str, account: &str) {
        secrets.delete(&pair_secret(host_id, account));
        if let Some(c) = self.kept_mut(host_id) {
            c.accounts.retain(|a| a != account);
            if c.account.as_deref() == Some(account) {
                c.account = None;
            }
        }
    }

    /// Retirer la console de cet appareil (ses associations aussi).
    pub fn remove(&mut self, secrets: &mut dyn Secrets, host_id: &str) {
        if let Some(c) = self.kept(host_id).cloned() {
            for a in &c.accounts {
                secrets.delete(&pair_secret(&c.host_id, a));
            }
        }
        self.file.consoles.retain(|c| !c.host_id.eq_ignore_ascii_case(host_id));
    }

    // MARK: - Stations (hors de chez soi)

    /// Une Station (sur le réseau) dit les consoles qu'elle voit : retenu
    /// pour chacune ; sa liste ne retire rien (une console éteinte ne répond
    /// plus). Vrai si quelque chose a changé.
    pub fn learn(&mut self, station: &str, host_ids: &[String]) -> bool {
        let mut changed = false;
        for h in host_ids {
            let list = self.file.seen_by.entry(h.to_ascii_uppercase()).or_default();
            if !list.iter().any(|s| s == station) {
                list.push(station.to_string());
                changed = true;
            }
        }
        changed
    }

    /// Les Stations qui voient cette console, dans l'ordre où elles l'ont vue.
    pub fn stations_of(&self, host_id: &str) -> &[String] {
        self.file.seen_by.get(&host_id.to_ascii_uppercase()).map_or(&[], |v| v.as_slice())
    }

    /// Un ordinateur retiré de l'app ne voit plus rien.
    pub fn forget_station(&mut self, station: &str) {
        for list in self.file.seen_by.values_mut() {
            list.retain(|s| s != station);
        }
        self.file.seen_by.retain(|_, v| !v.is_empty());
    }

    // MARK: - Vue

    /// Ce que l'interface reçoit. `away` : les Stations joignables par le
    /// relais en ce moment ; `sessions` : les consoles en partie.
    pub fn view(&self, now: Instant, away: &dyn Fn(&str) -> bool, sessions: &[String]) -> StateView {
        let accounts = self
            .file
            .accounts
            .iter()
            .map(|a| AccountView { id: a.id.clone(), name: a.name.clone(), consoles: self.consoles_of(&a.id) })
            .collect();
        let consoles = self
            .file
            .consoles
            .iter()
            .map(|c| {
                let id = console_id(&c.host_id);
                let away_station = self.stations_of(&c.host_id).iter().any(|s| away(s));
                let live = self.live(&c.host_id, now).cloned();
                ConsoleView {
                    in_session: sessions.contains(&id),
                    id,
                    host_id: c.host_id.clone(),
                    name: c.name.clone(),
                    model: c.model.clone(),
                    address: c.address.clone(),
                    status: self.status(c, now, away_station),
                    running_app: live.as_ref().and_then(|f| f.running_app.clone()).filter(|s| !s.trim().is_empty()),
                    accounts: self.paired_accounts(c).into_iter().map(|a| AccountRef { id: a.id.clone(), name: a.name.clone() }).collect(),
                    chosen: self.chosen(c),
                    free_accounts: self.free_accounts(&c.host_id).len(),
                    custom: c.settings().is_custom(),
                    waking: self.waking.iter().any(|h| h.eq_ignore_ascii_case(&c.host_id)),
                    reaching: self.reaching.iter().any(|h| h.eq_ignore_ascii_case(&c.host_id)),
                    found: live,
                }
            })
            .collect();
        StateView { accounts, consoles, found: self.on_network(now), searched: self.searched }
    }

    // MARK: - Envoi entre appareils

    /// Le paquet de « Envoyer à un appareil » : comptes (identifiant compris),
    /// consoles associées, leurs associations et les Stations qui les voient
    /// (`station` : l'ordinateur de l'app derrière un identifiant, ou rien).
    pub fn bundle(&self, from: &str, secrets: &dyn Secrets, station: &dyn Fn(&str) -> Option<BundleStation>) -> Bundle {
        let accounts = self
            .file
            .accounts
            .iter()
            .filter_map(|a| secrets.get(&account_secret(&a.id)).map(|psn_id| BundleAccount { id: a.id.clone(), name: a.name.clone(), psn_id }))
            .collect();
        let consoles = self
            .file
            .consoles
            .iter()
            .filter_map(|c| {
                let pairings: BTreeMap<String, String> =
                    c.accounts.iter().filter_map(|a| secrets.get(&pair_secret(&c.host_id, a)).map(|r| (a.clone(), r))).collect();
                if pairings.is_empty() {
                    return None;
                }
                let mut addresses = vec![c.address.clone()];
                addresses.retain(|a| !a.is_empty());
                Some(BundleConsole {
                    host_id: c.host_id.clone(),
                    name: c.name.clone(),
                    model: c.model.clone(),
                    addresses,
                    address: None,
                    accounts: pairings.keys().cloned().collect(),
                    target: (!c.target.is_empty()).then(|| c.target.clone()),
                    pairings,
                    stations: self.stations_of(&c.host_id).iter().filter_map(|s| station(s)).collect(),
                })
            })
            .collect();
        Bundle { from: from.to_string(), accounts, consoles }
    }

    /// Un paquet reçu : fusionné. Un compte au même identifiant PSN qu'un
    /// compte gardé est ce compte (son nom pris s'il n'en avait pas), sinon un
    /// nouveau compte (avec l'identifiant du paquet s'il est libre) ; une
    /// console au même `host_id` est cette console ; les associations
    /// s'ajoutent (une association déjà là reste). Stations : reliées aux
    /// ordinateurs de l'app par `station` (identifiant, empreinte, adresse),
    /// inconnues ignorées. Rend les nombres du paquet (comptes, consoles).
    pub fn merge(&mut self, secrets: &mut dyn Secrets, b: &Bundle, station: &dyn Fn(&BundleStation) -> Option<String>) -> (usize, usize) {
        // Identifiant du paquet → identifiant d'ici.
        let mut ids: HashMap<String, String> = HashMap::new();
        let mut fresh: Vec<String> = Vec::new();
        for a in &b.accounts {
            if valid_psn_id(&a.psn_id).is_none() {
                continue;
            }
            let same = self.file.accounts.iter_mut().find(|k| secrets.get(&account_secret(&k.id)).as_deref() == Some(a.psn_id.trim()));
            let id = match same {
                Some(k) => {
                    if k.name.is_none() {
                        k.name = a.name.clone();
                    }
                    k.id.clone()
                }
                None => {
                    let free = !a.id.trim().is_empty() && !self.file.accounts.iter().any(|k| k.id == a.id);
                    let id = if free { a.id.to_ascii_uppercase() } else { new_uuid() };
                    if !secrets.set(&account_secret(&id), a.psn_id.trim()) {
                        continue;
                    }
                    self.file.accounts.push(Account { id: id.clone(), name: a.name.clone() });
                    id
                }
            };
            ids.insert(a.id.clone(), id);
        }
        for c in &b.consoles {
            let host_id = c.host_id.to_ascii_uppercase();
            if host_id.is_empty() {
                continue;
            }
            let pairs: Vec<(String, &String)> = c.pairings.iter().filter_map(|(a, r)| ids.get(a).map(|id| (id.clone(), r))).collect();
            if pairs.is_empty() && self.kept(&host_id).is_none() {
                continue;
            }
            if self.kept(&host_id).is_none() {
                fresh.push(host_id.clone());
                self.file.consoles.push(Kept {
                    host_id: host_id.clone(),
                    name: c.name.clone(),
                    model: c.model.to_ascii_uppercase(),
                    address: c.all_addresses().first().cloned().unwrap_or_default(),
                    target: c.target.clone().unwrap_or_default(),
                    accounts: Vec::new(),
                    account: None,
                    settings: None,
                    layout: None,
                });
            }
            let mut added = Vec::new();
            for (id, registered) in pairs {
                let key = pair_secret(&host_id, &id);
                let have = self.kept(&host_id).is_some_and(|k| k.accounts.contains(&id)) && secrets.get(&key).is_some();
                if !have && secrets.set(&key, registered) {
                    added.push(id);
                }
            }
            if let Some(k) = self.kept_mut(&host_id) {
                for id in added {
                    if !k.accounts.contains(&id) {
                        k.accounts.push(id);
                    }
                }
                if k.address.is_empty() {
                    k.address = c.all_addresses().first().cloned().unwrap_or_default();
                }
                if k.target.is_empty() {
                    k.target = c.target.clone().unwrap_or_default();
                }
            }
            // Les autres adresses retenues : cherchées en direct ensuite.
            for a in c.all_addresses().iter().skip(1) {
                if !self.file.addresses.contains(a) {
                    self.file.addresses.push(a.clone());
                }
            }
            let stations: Vec<String> = c.stations.iter().filter_map(station).collect();
            self.learn_all(&host_id, &stations);
        }
        // Une console du paquet dont aucune association n'a pu être prise ici.
        self.file.consoles.retain(|c| !c.accounts.is_empty() || !fresh.contains(&c.host_id));
        (b.accounts.len(), b.consoles.len())
    }

    fn learn_all(&mut self, host_id: &str, stations: &[String]) {
        for s in stations {
            self.learn(s, &[host_id.to_string()]);
        }
    }
}

/// Le paquet de « Envoyer à un appareil » (chiffré par `playstation::handoff`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Bundle {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub accounts: Vec<BundleAccount>,
    #[serde(default)]
    pub consoles: Vec<BundleConsole>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleAccount {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// Toléré : `account`, `account_id` (proposition de l'app webOS).
    #[serde(alias = "account", alias = "account_id")]
    pub psn_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleConsole {
    pub host_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub addresses: Vec<String>,
    /// Toléré : une seule adresse en texte.
    #[serde(default, skip_serializing)]
    pub address: Option<String>,
    #[serde(default)]
    pub accounts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Compte → JSON `Registered` du cœur, en texte. Toléré : l'objet lui-même,
    /// ou un tableau `[{"account","registered"}]`.
    #[serde(default, deserialize_with = "pairings_any")]
    pub pairings: BTreeMap<String, String>,
    /// Toléré : les seuls identifiants, en texte.
    #[serde(default, deserialize_with = "stations_any")]
    pub stations: Vec<BundleStation>,
}

impl BundleConsole {
    /// Ses adresses, `address` comprise.
    fn all_addresses(&self) -> Vec<String> {
        let mut v = self.addresses.clone();
        if let Some(a) = self.address.as_ref().filter(|a| !a.is_empty() && !v.contains(a)) {
            v.push(a.clone());
        }
        v
    }
}

fn as_text(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(_) => Some(v.to_string()),
        _ => None,
    }
}

fn pairings_any<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeMap<String, String>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    let mut out = BTreeMap::new();
    match v {
        serde_json::Value::Object(map) => {
            for (k, r) in map {
                if let Some(t) = as_text(&r) {
                    out.insert(k, t);
                }
            }
        }
        serde_json::Value::Array(list) => {
            for e in list {
                if let (Some(a), Some(r)) = (e.get("account").and_then(|a| a.as_str()), e.get("registered").and_then(as_text)) {
                    out.insert(a.to_string(), r);
                }
            }
        }
        _ => {}
    }
    Ok(out)
}

fn stations_any<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<BundleStation>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    let serde_json::Value::Array(list) = v else { return Ok(Vec::new()) };
    Ok(list
        .into_iter()
        .filter_map(|e| match e {
            serde_json::Value::String(id) => Some(BundleStation { id, name: String::new(), address: String::new(), fingerprint: None }),
            other => serde_json::from_value(other).ok(),
        })
        .collect())
}

/// Une Station qui voit la console, telle que l'appareil qui envoie la connaît.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleStation {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub fingerprint: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Mem(HashMap<String, String>);

    impl Secrets for Mem {
        fn get(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set(&mut self, key: &str, value: &str) -> bool {
            self.0.insert(key.to_string(), value.to_string());
            true
        }
        fn delete(&mut self, key: &str) {
            self.0.remove(key);
        }
    }

    fn found(host: &str, state: &str) -> Found {
        Found {
            host_id: host.into(),
            name: "PS5 du salon".into(),
            model: "PS5".into(),
            address: "192.168.1.110".into(),
            state: state.into(),
            target: "ps5_v1".into(),
            running_app: Some("Astro Bot".into()),
        }
    }

    #[test]
    fn uuid_comme_swift() {
        let u = new_uuid();
        assert_eq!(u.len(), 36);
        assert_eq!(u, u.to_ascii_uppercase());
        assert_eq!(&u[14..15], "4");
        assert_ne!(u, new_uuid());
    }

    #[test]
    fn identifiant_psn() {
        assert_eq!(valid_psn_id(" AAAAAAAAAAA= ").as_deref(), Some("AAAAAAAAAAA="));
        assert!(valid_psn_id("AAAA").is_none());
        assert!(valid_psn_id("pas du base64").is_none());
    }

    #[test]
    fn comptes_sans_doublon() {
        let mut s = Mem::default();
        let mut c = Consoles::default();
        let (a, existed) = c.keep_account(&mut s, "AAAAAAAAAAA=", Some(" Matthieu_PSN ")).unwrap();
        assert!(!existed);
        assert_eq!(a.name.as_deref(), Some("Matthieu_PSN"));
        let (b, existed) = c.keep_account(&mut s, "AAAAAAAAAAA=", Some("Nouveau")).unwrap();
        assert!(existed);
        assert_eq!(a.id, b.id);
        assert_eq!(c.file.accounts.len(), 1);
        assert_eq!(c.file.accounts[0].name.as_deref(), Some("Nouveau"));
        // Sans nom : le nom gardé reste.
        c.keep_account(&mut s, "AAAAAAAAAAA=", None).unwrap();
        assert_eq!(c.file.accounts[0].name.as_deref(), Some("Nouveau"));
    }

    #[test]
    fn associer_choisir_retirer() {
        let mut s = Mem::default();
        let mut c = Consoles::default();
        let (a, _) = c.keep_account(&mut s, "AAAAAAAAAAA=", Some("A")).unwrap();
        let (b, _) = c.keep_account(&mut s, "AQEBAQEBAQE=", Some("B")).unwrap();
        let f = found("abcdef012345", "ready");
        assert!(c.paired(&mut s, &f, &f.host_id, "{\"k\":1}", &a.id));
        assert_eq!(c.free_accounts("ABCDEF012345").len(), 1);
        assert!(c.paired(&mut s, &f, &f.host_id, "{\"k\":2}", &b.id));
        let k = c.kept("ABCDEF012345").unwrap().clone();
        assert_eq!(k.accounts, vec![a.id.clone(), b.id.clone()]);
        // Le dernier associé est choisi.
        assert_eq!(c.chosen(&k).as_deref(), Some(b.id.as_str()));
        assert_eq!(s.get(&pair_secret("abcdef012345", &a.id)).as_deref(), Some("{\"k\":1}"));
        // Retirer A : la console reste avec B.
        c.remove_account(&mut s, &a.id);
        assert_eq!(c.file.consoles.len(), 1);
        assert!(s.get(&pair_secret("ABCDEF012345", &a.id)).is_none());
        assert!(s.get(&account_secret(&a.id)).is_none());
        // Plus reconnue par B : la console reste, « Associer » revient.
        c.unpair(&mut s, "ABCDEF012345", &b.id);
        assert!(c.kept("ABCDEF012345").unwrap().accounts.is_empty());
        // Retirer B : la console ne lui était plus associée, elle reste (« Associer »).
        c.remove_account(&mut s, &b.id);
        assert_eq!(c.file.consoles.len(), 1);
        assert!(c.file.accounts.is_empty());
        // Associée de nouveau puis son seul compte retiré : elle part.
        let (d, _) = c.keep_account(&mut s, "AgICAgICAgI=", None).unwrap();
        c.paired(&mut s, &f, &f.host_id, "{}", &d.id);
        c.remove_account(&mut s, &d.id);
        assert!(c.file.consoles.is_empty());
    }

    #[test]
    fn etat_vu_d_ici() {
        let mut s = Mem::default();
        let mut c = Consoles::default();
        let (a, _) = c.keep_account(&mut s, "AAAAAAAAAAA=", None).unwrap();
        let f = found("ABCDEF012345", "standby");
        c.paired(&mut s, &f, &f.host_id, "{}", &a.id);
        let k = c.kept("ABCDEF012345").unwrap().clone();
        let t0 = Instant::now();
        assert_eq!(c.status(&k, t0, false), Status::Searching);
        c.searched = true;
        assert_eq!(c.status(&k, t0, false), Status::Offline);
        assert_eq!(c.status(&k, t0, true), Status::Away);
        let mut moved = f.clone();
        moved.address = "192.168.1.120".into();
        assert!(c.record(vec![moved], t0));
        assert_eq!(c.kept("ABCDEF012345").unwrap().address, "192.168.1.120");
        assert_eq!(c.status(&k, t0, true), Status::Standby);
        assert_eq!(c.status(&k, t0 + Duration::from_secs(13), false), Status::Offline);
        let v = c.view(t0, &|_| false, &["ps-ABCDEF012345".into()]);
        assert_eq!(v.consoles[0].id, "ps-ABCDEF012345");
        assert!(v.consoles[0].in_session);
        assert_eq!(v.consoles[0].running_app.as_deref(), Some("Astro Bot"));
        assert_eq!(v.accounts[0].consoles, 1);
        assert_eq!(v.found.len(), 1);
    }

    #[test]
    fn stations_retenues() {
        let mut c = Consoles::default();
        assert!(c.learn("pc1", &["abcdef012345".into()]));
        assert!(!c.learn("pc1", &["ABCDEF012345".into()]));
        assert!(c.learn("pc2", &["ABCDEF012345".into()]));
        assert_eq!(c.stations_of("abcdef012345"), ["pc1", "pc2"]);
        c.forget_station("pc1");
        assert_eq!(c.stations_of("ABCDEF012345"), ["pc2"]);
    }

    #[test]
    fn envoi_et_fusion() {
        let mut s1 = Mem::default();
        let mut from = Consoles::default();
        let (a, _) = from.keep_account(&mut s1, "AAAAAAAAAAA=", Some("Matthieu_PSN")).unwrap();
        let f = found("ABCDEF012345", "ready");
        from.paired(&mut s1, &f, &f.host_id, "{\"reg\":1}", &a.id);
        from.learn("pc-salon", &["ABCDEF012345".into()]);
        let station = |id: &str| (id == "pc-salon").then(|| BundleStation { id: id.into(), name: "PC du salon".into(), address: "192.168.1.10".into(), fingerprint: None });
        let b = from.bundle("PC de Léa", &s1, &station);
        let json = serde_json::to_string(&b).unwrap();
        assert!(json.contains("\"psn_id\":\"AAAAAAAAAAA=\"") && json.contains("\"pairings\""));
        let b: Bundle = serde_json::from_str(&json).unwrap();

        // L'autre appareil a déjà le même compte, sous un autre identifiant et sans nom.
        let mut s2 = Mem::default();
        let mut to = Consoles::default();
        let (mine, _) = to.keep_account(&mut s2, "AAAAAAAAAAA=", None).unwrap();
        let (n_accounts, n_consoles) = to.merge(&mut s2, &b, &|st| (st.address == "192.168.1.10").then(|| "pc-ici".to_string()));
        assert_eq!((n_accounts, n_consoles), (1, 1));
        assert_eq!(to.file.accounts.len(), 1);
        assert_eq!(to.file.accounts[0].name.as_deref(), Some("Matthieu_PSN"));
        let k = to.kept("ABCDEF012345").unwrap();
        assert_eq!(k.accounts, vec![mine.id.clone()]);
        assert_eq!(s2.get(&pair_secret("ABCDEF012345", &mine.id)).as_deref(), Some("{\"reg\":1}"));
        assert_eq!(to.stations_of("ABCDEF012345"), ["pc-ici"]);
        // Reçu deux fois : rien en double.
        to.merge(&mut s2, &b, &|_| None);
        assert_eq!(to.file.accounts.len(), 1);
        assert_eq!(to.file.consoles.len(), 1);
        assert_eq!(to.kept("ABCDEF012345").unwrap().accounts.len(), 1);

        // Un appareil vide prend l'identifiant du paquet.
        let mut s3 = Mem::default();
        let mut empty = Consoles::default();
        empty.merge(&mut s3, &b, &|_| None);
        assert_eq!(empty.file.accounts[0].id, a.id);
    }

    #[test]
    fn paquet_des_autres_apps() {
        // Variante de l'app webOS : associations en objets, Stations en texte, `account`, `address`.
        let json = r#"{"version":1,"accounts":[{"id":"U1","name":null,"account":"AAAAAAAAAAA="}],
            "consoles":[{"host_id":"abcdef012345","name":"PS5","model":"ps5","address":"10.0.0.2",
            "stations":["pc1"],"accounts":["U1"],"pairings":{"U1":{"mac":"abcdef012345"}}}]}"#;
        let b: Bundle = serde_json::from_str(json).unwrap();
        assert_eq!(b.accounts[0].psn_id, "AAAAAAAAAAA=");
        assert_eq!(b.consoles[0].pairings["U1"], "{\"mac\":\"abcdef012345\"}");
        assert_eq!(b.consoles[0].stations[0].id, "pc1");
        let mut s = Mem::default();
        let mut c = Consoles::default();
        c.merge(&mut s, &b, &|st| Some(st.id.clone()));
        let k = c.kept("ABCDEF012345").unwrap();
        assert_eq!((k.model.as_str(), k.address.as_str()), ("PS5", "10.0.0.2"));
        assert_eq!(c.stations_of("ABCDEF012345"), ["pc1"]);
        // Tableau d'associations.
        let b: Bundle = serde_json::from_str(r#"{"consoles":[{"host_id":"A","pairings":[{"account":"U","registered":"{}"}]}]}"#).unwrap();
        assert_eq!(b.consoles[0].pairings["U"], "{}");
    }

    #[test]
    fn json_tolerant() {
        let c: PsFile = serde_json::from_str("{\"consoles\":[{\"host_id\":\"AB\",\"name\":\"x\",\"model\":\"PS4\",\"address\":\"\"}]}").unwrap();
        assert_eq!(c.consoles[0].target_or_default(), "ps4_v10");
        assert!(c.consoles[0].settings().fullscreen);
        assert_eq!(host_of("ps-ABC"), Some("ABC"));
    }
}
