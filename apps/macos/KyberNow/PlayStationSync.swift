// Consoles PlayStation sur plusieurs appareils (docs/playstation.md,
// « Plusieurs appareils ») : le format partagé des comptes et des consoles,
// par iCloud entre appareils Apple et par le réseau vers n'importe quel
// appareil (« Envoyer à un appareil »), et les règles de la synchro iCloud.
//
// FORMAT PARTAGÉ — les apps Apple TV, iPhone et iPad lisent et écrivent
// exactement ceci (et les autres apps, le paquet de l'envoi) :
//
// 1. Magasin clé-valeur d'iCloud, clé `playstation`, valeur `Data` (JSON en
//    UTF-8, comme la clé `hosts`) :
//      {"accounts":[{"id":"<UUID>","name":"<pseudo>"}],
//       "consoles":[{"host_id":"<HOSTID>","name":"…","model":"PS5","addresses":["…"],
//                    "accounts":["<UUID>"],"target":"ps5_v1"}]}
//    - `<UUID>` : `uuidString` (majuscules), le même sur tous les appareils Apple ;
//    - `<HOSTID>` : identifiant de la console (`mac` de l'association), majuscules ;
//    - `name` d'un compte absent : compte d'une version précédente (« Compte PlayStation ») ;
//    - `target` facultatif (pour réassocier) ; une console n'y est que si un compte au moins y est associé.
// 2. Trousseau iCloud : éléments synchronisables, service
//    `fr.leslignesclaires.kybernow.cloud.playstation`, compte
//    `account.<UUID>` → l'identifiant PSN (base64) et `<HOSTID>.<UUID>` →
//    l'association (JSON `Registered` du cœur, tel que `kn_ps_regist` le rend).
// 3. Paquet de l'envoi par le réseau (chiffré par le cœur, `playstation::handoff`) :
//    le JSON du 1., plus `from` (nom de l'appareil qui envoie), `psn_id` dans
//    chaque compte, `pairings` ({"<UUID>":"<JSON Registered en texte>"}) et
//    `stations` ([{"id","name","address","fingerprint"}]) dans chaque console.
//
// Règles de la synchro iCloud : `PSCloudPlan.make` ci-dessous.

import Foundation

/// Comptes et consoles, au format partagé.
struct PSShared: Codable, Equatable {
    struct Account: Codable, Equatable {
        var id: String
        var name: String?
        /// Identifiant PSN (base64) : dans le paquet de l'envoi seulement.
        var psnID: String?

        private enum CodingKeys: String, CodingKey {
            case id, name
            case psnID = "psn_id"
        }
    }

    /// Un ordinateur (une Station) qui voit la console, vu de l'appareil qui envoie.
    struct Station: Codable, Equatable {
        var id: String?
        var name: String?
        var address: String?
        /// Empreinte du certificat d'Admin.
        var fingerprint: String?
    }

    struct Console: Codable, Equatable {
        var hostID: String
        var name: String
        var model: String
        var addresses: [String]
        var accounts: [String]
        var target: String?
        /// Paquet de l'envoi seulement : l'association de chaque compte.
        var pairings: [String: String]?
        /// Paquet de l'envoi seulement.
        var stations: [Station]?

        private enum CodingKeys: String, CodingKey {
            case name, model, addresses, accounts, target, pairings, stations
            case hostID = "host_id"
        }

        init(hostID: String, name: String, model: String, addresses: [String], accounts: [String], target: String?,
             pairings: [String: String]? = nil, stations: [Station]? = nil) {
            (self.hostID, self.name, self.model, self.addresses, self.accounts, self.target, self.pairings, self.stations) =
                (hostID.uppercased(), name, model, addresses, accounts, target, pairings, stations)
        }

        init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            hostID = try c.decode(String.self, forKey: .hostID).uppercased()
            name = (try? c.decodeIfPresent(String.self, forKey: .name)) ?? "PlayStation"
            model = (try? c.decodeIfPresent(String.self, forKey: .model)) ?? "PS5"
            addresses = (try? c.decodeIfPresent([String].self, forKey: .addresses)) ?? []
            accounts = ((try? c.decodeIfPresent([String].self, forKey: .accounts)) ?? []).map { $0.uppercased() }
            target = try? c.decodeIfPresent(String.self, forKey: .target)
            pairings = (try? c.decodeIfPresent([String: String].self, forKey: .pairings)).map {
                Dictionary($0.map { ($0.key.uppercased(), $0.value) }, uniquingKeysWith: { a, _ in a })
            }
            stations = try? c.decodeIfPresent([Station].self, forKey: .stations)
        }
    }

    var from: String?
    var accounts: [Account] = []
    var consoles: [Console] = []

    private enum CodingKeys: String, CodingKey { case from, accounts, consoles }

    init(from: String? = nil, accounts: [Account] = [], consoles: [Console] = []) {
        (self.from, self.accounts, self.consoles) = (from, accounts, consoles)
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        from = try? c.decodeIfPresent(String.self, forKey: .from)
        accounts = ((try? c.decodeIfPresent([Account].self, forKey: .accounts)) ?? []).map {
            Account(id: $0.id.uppercased(), name: $0.name, psnID: $0.psnID)
        }
        consoles = (try? c.decodeIfPresent([Console].self, forKey: .consoles)) ?? []
    }

    static func decode(_ data: Data) -> PSShared? { try? JSONDecoder().decode(PSShared.self, from: data) }

    var data: Data? {
        let e = JSONEncoder()
        e.outputFormatting = [.sortedKeys]
        return try? e.encode(self)
    }

    var text: String? { data.flatMap { String(data: $0, encoding: .utf8) } }

    /// La cible d'association d'une console sans `target` : celle de
    /// l'association, sinon la plus récente de son modèle.
    static func target(model: String, registered: String?) -> String {
        if let registered, let o = try? JSONSerialization.jsonObject(with: Data(registered.utf8)) as? [String: Any],
           let t = o["target"] as? String { return t }
        return model == "PS4" ? "ps4_v10" : "ps5_v1"
    }

    /// Clés de la synchro : `account.<UUID>` et `<HOSTID>.<UUID>` (aussi les
    /// comptes du Trousseau iCloud).
    static func accountKey(_ id: String) -> String { "account.\(id)" }
    static func pairKey(_ hostID: String, _ account: String) -> String { "\(hostID).\(account)" }
}

/// « {nom} · Apple TV » : la plateforme d'un appareil en attente d'un envoi.
enum PSPlatform {
    static func label(_ platform: String) -> String {
        switch platform.lowercased() {
        case "macos": return "Mac"
        case "ios": return "iPhone"
        case "ipados": return "iPad"
        case "tvos": return "Apple TV"
        case "windows": return "PC"
        case "linux": return "Linux"
        case "steamdeck": return "Steam Deck"
        case "raspberrypi": return "Raspberry Pi"
        case "android": return "Android"
        case "androidtv": return "Android TV"
        case "webos": return "LG"
        default: return platform
        }
    }
}

/// Ce que l'appareil a, avec ses secrets, vu par la synchro iCloud.
struct PSLocal: Equatable {
    struct Account: Equatable {
        var id: String
        var name: String?
        var psnID: String
    }

    struct Console: Equatable {
        var hostID: String
        var name: String
        var model: String
        var addresses: [String]
        var target: String?
        /// Compte → association.
        var pairings: [String: String]
    }

    var accounts: [Account]
    var consoles: [Console]

    var keys: Set<String> {
        var k = Set(accounts.map { PSShared.accountKey($0.id) })
        for c in consoles { for a in c.pairings.keys { k.insert(PSShared.pairKey(c.hostID, a)) } }
        return k
    }
}

/// Une synchro iCloud des consoles : ce qu'il faut faire ici et écrire dans
/// iCloud. Les règles, les mêmes sur tous les appareils Apple :
///
/// Clés : `account.<UUID>` (un compte) et `<HOSTID>.<UUID>` (une association).
/// `known` : les clés présentes ici et dans iCloud à la dernière synchro
/// (gardées dans les préférences), plus celles en attente (règle 1).
/// 0. Doublon : un compte d'iCloud U a le même identifiant PSN qu'un compte
///    d'ici L : le plus petit `uuidString` gagne. U < L : L est renommé U ici
///    (ses associations d'iCloud, même venues d'ailleurs, arrivent sous U).
///    L < U : U n'est pas repris, ses associations arrivent sous L ; U reste
///    dans iCloud (l'appareil qui le tient le renommera).
/// 1. Retiré ailleurs : dans `known`, ici, plus dans la liste → retiré ici
///    si son élément du Trousseau iCloud n'est plus là non plus ; sinon (un
///    renommage, ou le Trousseau en retard) : gardé ici, pas réécrit dans la
///    liste, toujours dans `known`, revu à la synchro suivante.
/// 2. Retiré ici : dans `known`, plus ici, encore dans la liste → retiré de
///    la liste (un compte emporte toutes ses associations) et du Trousseau
///    iCloud ; l'ancien identifiant d'un compte renommé : de la liste seulement.
/// 3. Nouveau ailleurs : dans la liste seulement, pas dans `known` → repris
///    si son élément du Trousseau iCloud est là, sinon laissé pour plus tard.
/// 4. Nouveau ici : ici seulement → ajouté à iCloud (liste et Trousseau).
/// 5. Liste écrite : liste d'iCloud − retirés ici + ce qui est ici (sauf
///    l'attente de la règle 1) ; une console connue ici : son nom, son modèle
///    et ses adresses d'ici.
/// 6. `known` = (ici ∩ liste écrite) + l'attente de la règle 1.
/// Un changement de compte iCloud ou la synchro coupée vident `known` (rien
/// n'est effacé ; couper la synchro ne retire rien d'iCloud).
struct PSCloudPlan {
    struct Pairing: Equatable {
        var console: PSShared.Console
        var account: String
        var registered: String
    }

    /// Comptes d'ici à renommer (règle 0), à faire en premier.
    var renames: [(from: String, to: String)] = []
    /// Comptes repris d'iCloud.
    var importAccounts: [PSLocal.Account] = []
    /// Associations reprises d'iCloud (le compte, après les renommages et reprises).
    var importPairings: [Pairing] = []
    /// Retirés ailleurs.
    var removeAccounts: [String] = []
    var removePairings: [(hostID: String, account: String)] = []
    /// La liste à écrire dans iCloud.
    var cloud = PSShared()
    /// Éléments du Trousseau iCloud à écrire (clé → valeur) et à effacer.
    var writeSecrets: [String: String] = [:]
    var deleteSecrets: Set<String> = []
    var known: Set<String> = []
    /// Des éléments attendent le Trousseau iCloud : réessayer plus tard.
    var pending = false

    /// Rien à faire ici.
    var quiet: Bool { renames.isEmpty && importAccounts.isEmpty && importPairings.isEmpty && removeAccounts.isEmpty && removePairings.isEmpty }

    /// `secret` lit un élément du Trousseau iCloud (`account.<UUID>`, `<HOSTID>.<UUID>`).
    static func make(local start: PSLocal, cloud: PSShared, known: Set<String>, secret: (String) -> String?) -> PSCloudPlan {
        var plan = PSCloudPlan()
        var local = start
        let accountKey = PSShared.accountKey, pairKey = PSShared.pairKey

        // 0. Doublons par identifiant PSN. `alias` : identifiant d'iCloud → celui d'ici.
        var alias: [String: String] = [:]
        var renamed: Set<String> = []
        let startIDs = Set(local.accounts.map(\.id))
        for u in cloud.accounts where !startIDs.contains(u.id) {
            guard let psn = secret(accountKey(u.id)),
                  let l = local.accounts.first(where: { $0.psnID == psn && $0.id != u.id }) else { continue }
            if u.id < l.id {
                plan.renames.append((l.id, u.id))
                alias[l.id] = u.id
                renamed.insert(l.id)
                for i in local.accounts.indices where local.accounts[i].id == l.id { local.accounts[i].id = u.id }
                for i in local.consoles.indices {
                    if let r = local.consoles[i].pairings.removeValue(forKey: l.id) { local.consoles[i].pairings[u.id] = r }
                }
            } else {
                alias[u.id] = l.id
            }
        }

        let localKeys = local.keys
        var cloudKeys = Set(cloud.accounts.map { accountKey($0.id) })
        for c in cloud.consoles { for a in c.accounts { cloudKeys.insert(pairKey(c.hostID, a)) } }

        // 1. Retirés ailleurs, ou en attente.
        var gone: Set<String> = [], waiting: Set<String> = []
        for k in known.intersection(localKeys).subtracting(cloudKeys) {
            if secret(k) == nil { gone.insert(k) } else { waiting.insert(k) }
        }
        plan.removeAccounts = local.accounts.map(\.id).filter { gone.contains(accountKey($0)) }
        local.accounts.removeAll { plan.removeAccounts.contains($0.id) }
        for i in local.consoles.indices {
            for a in local.consoles[i].pairings.keys.sorted()
            where plan.removeAccounts.contains(a) || gone.contains(pairKey(local.consoles[i].hostID, a)) {
                local.consoles[i].pairings[a] = nil
                if !plan.removeAccounts.contains(a) { plan.removePairings.append((local.consoles[i].hostID, a)) }
            }
        }
        local.consoles.removeAll { $0.pairings.isEmpty }

        // 2. Retirés ici : un compte emporte ses associations.
        let dropped = known.intersection(cloudKeys).subtracting(localKeys)
        let droppedAccounts = Set(cloud.accounts.map(\.id).filter { dropped.contains(accountKey($0)) })
        func isDropped(_ hostID: String, _ a: String) -> Bool { droppedAccounts.contains(a) || dropped.contains(pairKey(hostID, a)) }
        for k in dropped where !renamed.contains(String(k.split(separator: ".").last ?? "")) { plan.deleteSecrets.insert(k) }
        for c in cloud.consoles {
            for a in c.accounts where droppedAccounts.contains(a) && !renamed.contains(a) { plan.deleteSecrets.insert(pairKey(c.hostID, a)) }
        }

        // 3. Nouveaux ailleurs.
        for u in cloud.accounts where !droppedAccounts.contains(u.id) && alias[u.id] == nil && !local.accounts.contains(where: { $0.id == u.id }) {
            let key = accountKey(u.id)
            guard !known.contains(key) else { continue }
            guard let psn = secret(key) else { plan.pending = true; continue }
            let a = PSLocal.Account(id: u.id, name: u.name, psnID: psn)
            plan.importAccounts.append(a)
            local.accounts.append(a)
        }
        for c in cloud.consoles {
            for u in c.accounts {
                let key = pairKey(c.hostID, u)
                // Les associations de l'ancien identifiant d'un compte renommé ici
                // (venues d'autres appareils) arrivent sous le nouveau.
                guard !dropped.contains(key), !known.contains(key), !(droppedAccounts.contains(u) && !renamed.contains(u)) else { continue }
                let account = alias[u] ?? u
                guard local.accounts.contains(where: { $0.id == account }) else { continue }
                if let i = local.consoles.firstIndex(where: { $0.hostID == c.hostID }), local.consoles[i].pairings[account] != nil { continue }
                guard let registered = secret(key) else { plan.pending = true; continue }
                plan.importPairings.append(Pairing(console: c, account: account, registered: registered))
                if let i = local.consoles.firstIndex(where: { $0.hostID == c.hostID }) {
                    local.consoles[i].pairings[account] = registered
                } else {
                    local.consoles.append(PSLocal.Console(hostID: c.hostID, name: c.name, model: c.model, addresses: c.addresses,
                                                          target: c.target, pairings: [account: registered]))
                }
            }
        }

        // 4 et 5. La liste écrite : iCloud − retirés ici + ici (sauf l'attente).
        let published = local.accounts.filter { !waiting.contains(accountKey($0.id)) }
        let here = Set(local.accounts.map(\.id))
        let publishedIDs = Set(published.map(\.id))
        func mine(_ c: PSLocal.Console) -> [String] {
            c.pairings.keys.sorted().filter { publishedIDs.contains($0) && !waiting.contains(pairKey(c.hostID, $0)) }
        }
        var accounts = cloud.accounts.filter { !droppedAccounts.contains($0.id) && !here.contains($0.id) }
            .map { PSShared.Account(id: $0.id, name: $0.name) }
        accounts += published.map { PSShared.Account(id: $0.id, name: $0.name) }
        var consoles: [PSShared.Console] = []
        var seen: Set<String> = []
        for c in cloud.consoles where !seen.contains(c.hostID) {
            seen.insert(c.hostID)
            let others = c.accounts.filter { !isDropped(c.hostID, $0) && !here.contains($0) }
            if let l = local.consoles.first(where: { $0.hostID == c.hostID }) {
                consoles.append(PSShared.Console(hostID: l.hostID, name: l.name, model: l.model, addresses: l.addresses,
                                                 accounts: others + mine(l), target: l.target ?? c.target))
            } else {
                var kept = c
                kept.accounts = others
                kept.pairings = nil
                kept.stations = nil
                consoles.append(kept)
            }
        }
        for l in local.consoles where !seen.contains(l.hostID) {
            consoles.append(PSShared.Console(hostID: l.hostID, name: l.name, model: l.model, addresses: l.addresses,
                                             accounts: mine(l), target: l.target))
        }
        plan.cloud = PSShared(accounts: accounts, consoles: consoles.filter { !$0.accounts.isEmpty })

        // Trousseau iCloud : ce qui est publié d'ici.
        for a in published { plan.writeSecrets[accountKey(a.id)] = a.psnID }
        for c in local.consoles { for a in mine(c) { plan.writeSecrets[pairKey(c.hostID, a)] = c.pairings[a] } }

        // 6.
        var written = Set(plan.cloud.accounts.map { accountKey($0.id) })
        for c in plan.cloud.consoles { for a in c.accounts { written.insert(pairKey(c.hostID, a)) } }
        plan.known = local.keys.intersection(written).union(waiting)
        return plan
    }
}
