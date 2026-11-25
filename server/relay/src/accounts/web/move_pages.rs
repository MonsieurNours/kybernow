//! Déplacer un ordinateur dans la console (docs/comptes.md, « Réglages
//! synchronisés et déplacer un ordinateur ») : « Déplacer… » sur la page de
//! l'ordinateur (panneau `#deplacer`, la destination), puis « Ce qui change »,
//! puis « Déplacer {ordinateur} ». Le calcul et le déplacement sont dans
//! [`super::super::moves`].

use super::super::moves::{Dest, MoveRefusal};
use super::super::pages::{Notice, console};
use super::console::{Kept, not_found, people_page, shell, signed_in};
use super::device_pages::sign_in_first;
use super::{Cx, Reply};

/// `POST /compte/ordinateur/deplacer` : `id`, `vers` (`moi` ou `org_…`) ; sans
/// `action`, « Ce qui change » ; `action=deplacer` (et `garder=1` pour garder
/// les accès donnés à des personnes) : le déplacement.
pub(super) fn deplacer(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let id = cx.req.form("id");
    if !kybernow_tunnel::id::is_station_id(id) {
        return not_found(cx);
    }
    let station = acc.db().station(id).ok().flatten();
    let Some(station) = station else { return not_found(cx) };
    // Ni son propriétaire, ni gérant de sa famille ou de son équipe : comme un ordinateur inconnu,
    // sauf pour qui y a un accès (sa page, avec le refus).
    if acc.standing(&account, &station).is_none() {
        return not_found(cx);
    }
    let dest = match acc.move_check(&account, &station, cx.req.form("vers")) {
        Ok(d) => d,
        Err(refusal) => {
            let notice = match refusal {
                MoveRefusal::NotAllowed => Notice::NotAllowed,
                MoveRefusal::NeedTwoFactor => Notice::NeedTwoFactor,
                MoveRefusal::NotManager(name) => Notice::NotManager(name),
            };
            return people_page(cx, &account, &station, Some(notice), Kept::default());
        }
    };
    let dest_name = match &dest {
        Dest::Me => cx.tr("vous", "you"),
        Dest::Org(o) => o.name.clone(),
    };
    if cx.req.form("action") != "deplacer" {
        let view = console::MoveView {
            id: station.id.clone(),
            name: station.name.clone(),
            dest: match &dest {
                Dest::Me => "moi".into(),
                Dest::Org(o) => o.public_id.clone(),
            },
            dest_name,
            plan: acc.move_plan(&account, &station, &dest),
            shell: shell(&cx, &account),
        };
        let page = console::move_preview(&cx.view(), &view);
        return cx.page(200, page);
    }
    let keep = cx.req.form("garder") == "1";
    match acc.move_station(&account, &station, &dest, keep, &cx.ip()) {
        Ok(true) => {}
        Ok(false) => return not_found(cx),
        Err(e) => {
            tracing::warn!("déplacer un ordinateur : {e:#}");
            return cx.failed();
        }
    }
    let moved = acc.db().station(&station.id).ok().flatten();
    let Some(moved) = moved else { return not_found(cx) };
    people_page(cx, &account, &moved, Some(Notice::Moved(station.name.clone(), dest_name)), Kept::default())
}
