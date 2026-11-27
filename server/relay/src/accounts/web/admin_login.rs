//! « Se connecter avec KyberNow » dans Admin, sur le réseau seulement
//! (docs/comptes.md, « Admin sur le réseau », lot 4) : un compte qui a le
//! droit Administrer ouvre Admin sans son mot de passe.
//!
//! Admin envoie le navigateur ici : `GET /compte/ouvrir-admin?station=…&nonce=…&retour=<adresse d'Admin>`.
//! Connecté (double authentification faite), avec le droit Administrer sur
//! cette Station, on confirme « Ouvrir Admin de {ordinateur} ? » ; le relais
//! délivre un jeton d'accès (2 min) dont le `jti` est le `nonce` d'Admin
//! (lié au cookie du navigateur chez Admin), et renvoie vers
//! `<retour>/api/login/kybernow?token=…`. Admin le vérifie (clé épinglée,
//! droit Administrer, usage unique, `jti` = son `nonce`) et ouvre sa session.
//!
//! `retour` n'est qu'une adresse du réseau local (adresse IP privée, nom
//! sans point, `.local`, `.lan`, `.home.arpa`), en `http` ou `https`, sans
//! chemin : le jeton ne part pas vers un site quelconque. Quand la Station a
//! donné ses adresses d'Admin (`admin_lan`), une adresse IP n'est acceptée que
//! parmi elles (ou en boucle locale), et un nom seulement dans leur schéma
//! (REL-10) : un voisin du réseau ne fait pas partir le jeton vers sa machine.

use std::net::IpAddr;

use kybernow_tunnel::access::{Claims, Names, Right};
use kybernow_tunnel::id::is_station_id;

use super::super::pages::console;
use super::super::pages::Notice;
use super::console::signed_in;
use super::device_pages::sign_in_first;
use super::{Cx, Reply};

/// Le `nonce` d'Admin : 16 octets en base32 minuscule (26 signes).
fn nonce_ok(n: &str) -> bool {
    n.len() == 26 && n.bytes().all(|b| matches!(b, b'a'..=b'z' | b'2'..=b'7'))
}

/// Une adresse IP du réseau local (privée, lien local, boucle locale).
fn local_ip(ip: IpAddr) -> bool {
    match ip.to_canonical() {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local() || v4.is_loopback() || (v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1])),
        IpAddr::V6(v6) => v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00 || (v6.segments()[0] & 0xffc0) == 0xfe80,
    }
}

/// `retour` : l'adresse d'Admin sur le réseau local, `http(s)://hôte[:port]` ; `None` sinon.
pub fn local_origin(retour: &str) -> Option<String> {
    let retour = retour.trim().trim_end_matches('/');
    let (scheme, rest) = retour.split_once("://")?;
    if !matches!(scheme, "http" | "https") || rest.is_empty() || rest.contains(['/', '?', '#', '@', '\\', ' ']) || retour.len() > 200 {
        return None;
    }
    // L'hôte, sans le port (`[fe80::1]:8443`, `192.168.1.20:8443`, `pc-salon.local`).
    let host = if let Some(v6) = rest.strip_prefix('[') {
        let (h, after) = v6.split_once(']')?;
        if !(after.is_empty() || after.strip_prefix(':').is_some_and(|p| p.parse::<u16>().is_ok())) {
            return None;
        }
        h.to_string()
    } else {
        let (h, port) = match rest.rsplit_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (rest, None),
        };
        if port.is_some_and(|p| p.parse::<u16>().is_err()) {
            return None;
        }
        h.to_ascii_lowercase()
    };
    let ok = match host.parse::<IpAddr>() {
        Ok(ip) => local_ip(ip),
        Err(_) => {
            !host.is_empty()
                && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
                && (!host.contains('.') || host.ends_with(".local") || host.ends_with(".lan") || host.ends_with(".home.arpa"))
        }
    };
    ok.then(|| format!("{scheme}://{rest}"))
}

/// `retour`, s'il est l'une des adresses d'Admin données par la Station (`published`, vide :
/// inconnues, la règle de [`local_origin`] seule) ; sinon une adresse en boucle locale, ou un nom
/// du réseau dans le schéma de ces adresses. Une adresse IP du réseau qu'Admin n'a pas donnée
/// est refusée (REL-10).
pub fn admin_origin(retour: &str, published: &[String]) -> Option<String> {
    let origin = local_origin(retour)?;
    if published.is_empty() || published.iter().any(|p| p.trim_end_matches('/') == origin) {
        return Some(origin);
    }
    let (scheme, rest) = origin.split_once("://")?;
    let host = match rest.strip_prefix('[') {
        Some(v6) => v6.split_once(']')?.0.to_string(),
        None => rest.rsplit_once(':').map(|(h, _)| h).unwrap_or(rest).to_string(),
    };
    match host.parse::<IpAddr>() {
        Ok(ip) => ip.to_canonical().is_loopback().then_some(origin),
        Err(_) => {
            let same_scheme = published.iter().any(|p| p.starts_with(&format!("{scheme}://")));
            (host == "localhost" || same_scheme).then_some(origin)
        }
    }
}

fn refused(cx: Cx<'_>, text_fr: &str, text_en: &str) -> Reply {
    let page = super::super::pages::message(
        &cx.view(),
        "/compte/ouvrir-admin",
        &cx.tr("Admin ne s'ouvre pas", "Admin doesn't open"),
        &cx.tr(text_fr, text_en),
        ("/compte/ordinateurs", &cx.tr("Vos ordinateurs", "Your computers")),
    );
    cx.page(400, page)
}

/// `GET/POST /compte/ouvrir-admin`.
pub(super) fn open_admin(cx: Cx<'_>, post: bool) -> Reply {
    let get = |k: &str| if post { cx.req.form(k).to_string() } else { cx.req.query(k).unwrap_or("").to_string() };
    let (station_id, nonce, retour) = (get("station"), get("nonce"), get("retour"));
    if !is_station_id(&station_id) || !nonce_ok(&nonce) {
        return refused(cx, "Cette demande ne vient pas d'Admin.", "This request doesn't come from Admin.");
    }
    // Sur le réseau (une adresse locale), ou à distance : exactement l'adresse de cette Station par le
    // relais, de bout en bout (`<id>.<tls_zone>`, quand elle tient son certificat) ou déchiffrée.
    let acc = cx.acc;
    let asked = retour.trim().trim_end_matches('/');
    let e2e_origin = acc.stations.tls_origin(&station_id);
    let e2e = e2e_origin.as_deref() == Some(asked);
    let remote = e2e || asked == acc.legacy_origin(&station_id);
    let published = acc.stations.admin(&station_id).map(|(lan, _)| lan).unwrap_or_default();
    let Some(origin) = (if remote { Some(asked.to_string()) } else { admin_origin(&retour, &published) }) else {
        return refused(
            cx,
            "Cette adresse n'est ni celle d'Admin sur le réseau local, ni celle de cet ordinateur par KyberNow Share.",
            "This address is neither Admin's on the local network, nor this computer's through KyberNow Share.",
        );
    };
    if remote && !acc.stations.admin(&station_id).is_some_and(|(_, on)| on) {
        return refused(
            cx,
            "Admin à distance est coupé sur cet ordinateur (ou il est hors ligne). Allumez-le dans Admin › Hors de chez vous.",
            "Remote Admin is off on this computer (or it's offline). Turn it on in Admin › Away from home.",
        );
    }
    let account = signed_in!(cx);
    let station = acc.db().station(&station_id).ok().flatten();
    let standing = station.as_ref().and_then(|s| acc.standing(&account, s));
    let Some((station, standing)) = station.zip(standing).filter(|(_, s)| s.rights.has(Right::Admin)) else {
        let needs = station_id_needs_2fa(&cx, &account, &station_id);
        if needs {
            let page = super::super::pages::message(
                &cx.view(),
                "/compte/ouvrir-admin",
                &cx.tr("Admin ne s'ouvre pas", "Admin doesn't open"),
                &Notice::NeedTwoFactor.text(cx.lang),
                ("/compte/double-authentification", &cx.tr("Activer la double authentification", "Turn on two-factor authentication")),
            );
            return cx.page(403, page);
        }
        return refused(
            cx,
            "Votre compte n'a pas le droit Administrer sur cet ordinateur.",
            "Your account doesn't have the Administer right on this computer.",
        );
    };
    // À distance : une passkey du compte confirme chaque ouverture.
    let allow: Vec<String> = if remote { acc.db().passkeys(account.id).unwrap_or_default().iter().map(|p| super::super::b64(&p.credential)).collect() } else { Vec::new() };
    let show = |cx: Cx<'_>, status: u16| {
        let host = origin.split_once("://").map(|(_, h)| h).unwrap_or(&origin).to_string();
        let page = console::open_admin(&cx.view(), &station.name, &host, &[("station", &station_id), ("nonce", &nonce), ("retour", &origin)], remote.then_some(allow.as_slice()));
        cx.page(status, page)
    };
    if !post {
        return show(cx, 200);
    }
    if cx.req.form("action") != "ouvrir" {
        let page = super::super::pages::done(&cx.view(), "/compte/ouvrir-admin", &cx.tr("Annulé", "Cancelled"), &cx.tr("Admin ne s'ouvre pas.", "Admin doesn't open."));
        return cx.page(200, page);
    }
    if remote && (allow.is_empty() || cx.check_passkey(Some(account.id)).is_err()) {
        cx.audit(Some(account.id), "admin_distance_refuse");
        return show(cx, 403);
    }
    let names = Names { account: account.name.clone(), email: Some(account.email.clone()), device: cx.tr("Navigateur (Admin sur le réseau)", "Browser (Admin on the network)") };
    let mut claims = Claims::issue(&acc.relay_name(), &station.id, &acc.public_account_id(account.id), "web", standing.rights, names, cx.now.max(0) as u64);
    // Le `jti` est le `nonce` d'Admin : le jeton ne sert qu'au navigateur qui l'a demandé.
    claims.jti = nonce;
    // À distance, après la passkey : Admin n'ouvre sa session par le relais qu'avec ce jeton.
    if remote {
        claims.via = Some("relay".into());
        claims.name.device = cx.tr("Navigateur (Admin à distance)", "Browser (remote Admin)");
    }
    let token = acc.stations.keys.signer().sign(&claims);
    cx.audit(Some(account.id), "admin_ouvert");
    if let Some(org) = station.org {
        let detail = serde_json::json!({ "station": station.name, "device": "Admin" });
        let _ = acc.db().org_log(org, Some(&account), "acces", detail, Some(&cx.ip()), cx.now);
    }
    tracing::info!(station = %station.id, "Admin ouvert avec un compte KyberNow");
    // De bout en bout, la Station est au moins une 0.11.8 : le jeton part dans le fragment (jamais
    // envoyé au serveur ni gardé dans un journal, REL-10) ; une Station 0.11.7 ne lit que `?token=`.
    let sep = if e2e { '#' } else { '?' };
    let page = console::open_admin_go(&cx.view(), &format!("{origin}/api/login/kybernow{sep}token={token}"));
    cx.page(200, page)
}

/// `GET /compte/admin?id=…` : « Ouvrir Admin » de la console. À distance si la Station le permet,
/// sinon sur son réseau ; le départ est chez Admin (`/api/login/kybernow/start`), qui tire le `nonce`.
pub(super) fn start(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let id = cx.req.query("id").unwrap_or("").to_string();
    let station = if is_station_id(&id) { acc.db().station(&id).ok().flatten() } else { None };
    let Some(station) = station.filter(|s| acc.standing(&account, s).is_some_and(|st| st.rights.has(Right::Admin))) else {
        return refused(cx, "Votre compte n'a pas le droit Administrer sur cet ordinateur.", "Your account doesn't have the Administer right on this computer.");
    };
    let Some((lan, remote_on)) = acc.stations.admin(&station.id) else {
        return refused(cx, "Cet ordinateur est hors ligne : Admin ne s'ouvre pas.", "This computer is offline: Admin doesn't open.");
    };
    let q = format!("?lang={}", cx.lang.code());
    if remote_on {
        let target = format!("{}/api/login/kybernow/start{q}", acc.station_origin(&station.id));
        return cx.redirect(target);
    }
    let lan = lan.first().map(|o| format!("{o}/api/login/kybernow/start{q}"));
    let page = console::admin_lan_only(&cx.view(), &station.name, lan.as_deref());
    cx.page(200, page)
}

/// Le droit Administrer est donné, mais sans double authentification.
fn station_id_needs_2fa(cx: &Cx<'_>, account: &super::super::store::Account, station: &str) -> bool {
    let row = cx.acc.db().station(station).ok().flatten();
    row.and_then(|r| cx.acc.standing(account, &r)).is_some_and(|s| s.admin_needs_2fa)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresses_d_admin_sur_le_reseau() {
        for ok in [
            "https://192.168.1.20:8443",
            "https://192.168.1.20:8443/",
            "http://10.0.0.5",
            "https://[fe80::1]:8443",
            "https://pc-salon.local:8443",
            "https://pc-salon:8443",
            "https://box.home.arpa",
            "https://100.64.1.2:8443",
        ] {
            assert!(local_origin(ok).is_some(), "{ok}");
        }
        for bad in [
            "https://exemple.org",
            "https://8.8.8.8",
            "https://192.168.1.20:8443/api",
            "javascript:alert(1)",
            "https://user@192.168.1.20",
            "ftp://192.168.1.20",
            "https://192.168.1.20:99999",
            "https://[2001:db8::1]:8443",
            "https://pc.local.exemple.org",
        ] {
            assert!(local_origin(bad).is_none(), "{bad}");
        }
        assert_eq!(local_origin("https://192.168.1.20:8443/").as_deref(), Some("https://192.168.1.20:8443"));
        assert!(nonce_ok("abcdefghijklmnopqrstuvwxyz") && !nonce_ok("ABCDEFGHIJKLMNOPQRSTUVWXYZ") && !nonce_ok("abc"));
    }

    #[test]
    fn seulement_les_adresses_qu_admin_a_donnees() {
        // REL-10 : la Station a donné https://192.168.1.20:8443.
        let published = vec!["https://192.168.1.20:8443".to_string()];
        assert_eq!(admin_origin("https://192.168.1.20:8443/", &published).as_deref(), Some("https://192.168.1.20:8443"));
        assert!(admin_origin("http://192.168.1.66", &published).is_none(), "la machine d'un voisin");
        assert!(admin_origin("https://192.168.1.66:8443", &published).is_none());
        assert!(admin_origin("https://192.168.1.20:9000", &published).is_none(), "autre port");
        assert!(admin_origin("https://127.0.0.1:8443", &published).is_some(), "Admin sur ce même ordinateur");
        assert!(admin_origin("https://localhost:8443", &published).is_some());
        assert!(admin_origin("https://pc-salon.local:8443", &published).is_some(), "par son nom, en https");
        assert!(admin_origin("http://pc-salon.local:8443", &published).is_none(), "pas en http quand Admin est en https");
        // Adresses inconnues (Station hors ligne, Admin ancien) : la règle du réseau local seule.
        assert!(admin_origin("http://192.168.1.66", &[]).is_some());
    }
}
