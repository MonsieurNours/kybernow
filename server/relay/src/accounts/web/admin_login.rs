//! « Se connecter avec KyberNow » dans Admin, sur le réseau seulement
//! (docs/comptes.md, « Admin sur le réseau », lot 4) : un compte qui a le
//! droit Administrer ouvre Admin sans son mot de passe.
//!
//! Admin envoie le navigateur ici : `GET /compte/ouvrir-admin?station=…&nonce=…&retour=<adresse d'Admin>`.
//! Connecté (double authentification faite), avec le droit Administrer sur
//! cette Station, on confirme « Ouvrir Admin de {ordinateur} ? » ; le relais
//! délivre un jeton d'accès (5 min) dont le `jti` est le `nonce` d'Admin
//! (lié au cookie du navigateur chez Admin), et renvoie vers
//! `<retour>/api/login/kybernow?token=…`. Admin le vérifie (clé épinglée,
//! droit Administrer, usage unique, `jti` = son `nonce`) et ouvre sa session.
//!
//! `retour` n'est qu'une adresse du réseau local (adresse IP privée, nom
//! sans point, `.local`, `.lan`, `.home.arpa`), en `http` ou `https`, sans
//! chemin : le jeton ne part pas vers un site quelconque.

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
    let Some(origin) = local_origin(&retour) else {
        return refused(
            cx,
            "Admin s'ouvre avec KyberNow sur le réseau seulement : cette adresse n'est pas une adresse du réseau local.",
            "Admin opens with KyberNow on the network only: this address isn't a local network address.",
        );
    };
    let account = signed_in!(cx);
    let acc = cx.acc;
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
    if !post {
        let host = origin.split_once("://").map(|(_, h)| h).unwrap_or(&origin).to_string();
        let page = console::open_admin(&cx.view(), &station.name, &host, &[("station", &station_id), ("nonce", &nonce), ("retour", &origin)]);
        return cx.page(200, page);
    }
    if cx.req.form("action") != "ouvrir" {
        let page = super::super::pages::done(&cx.view(), "/compte/ouvrir-admin", &cx.tr("Annulé", "Cancelled"), &cx.tr("Admin ne s'ouvre pas.", "Admin doesn't open."));
        return cx.page(200, page);
    }
    let names = Names { account: account.name.clone(), email: Some(account.email.clone()), device: cx.tr("Navigateur (Admin sur le réseau)", "Browser (Admin on the network)") };
    let mut claims = Claims::issue(&acc.relay_name(), &station.id, &acc.public_account_id(account.id), "web", standing.rights, names, cx.now.max(0) as u64);
    // Le `jti` est le `nonce` d'Admin : le jeton ne sert qu'au navigateur qui l'a demandé.
    claims.jti = nonce;
    let token = acc.stations.keys.signer().sign(&claims);
    cx.audit(Some(account.id), "admin_ouvert");
    if let Some(org) = station.org {
        let detail = serde_json::json!({ "station": station.name, "device": "Admin" });
        let _ = acc.db().org_log(org, Some(&account), "acces", detail, Some(&cx.ip()), cx.now);
    }
    tracing::info!(station = %station.id, "Admin ouvert avec un compte KyberNow");
    let page = console::open_admin_go(&cx.view(), &format!("{origin}/api/login/kybernow?token={token}"));
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
}
