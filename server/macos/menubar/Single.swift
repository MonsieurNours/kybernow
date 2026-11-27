// Une seule icône dans la barre des menus (bug du 27/11 : plusieurs icônes).
//
// Cause relevée dans le journal du Mac (26/11, 18:39) : l'app ouverte par
// Spotlight est lancée par LaunchServices (`application.fr.leslignesclaires…`),
// puis le service launchd de l'icône (`…station.menubar`, RunAtLoad) en lance
// une seconde, 22 s plus tard. De même, une icône ouverte à la main survit au
// `bootout` d'une mise à jour, et la nouvelle version en ajoute une.
//
// Règle : l'icône est celle du service launchd. Une copie lancée autrement
// (Finder, Spotlight, `open`) ne reste pas : elle démarre le service s'il ne
// tourne pas, ouvre Admin s'il tournait déjà, et s'arrête. Un verrou (`flock`)
// tranche les lancements simultanés ; le service qui trouve le verrou tenu par
// une copie d'avant ce correctif lui demande de s'arrêter (SIGTERM) et attend.

import AppKit

enum InstanceRole: Equatable {
    /// Garder cette copie : c'est l'icône.
    case run
    /// Démarrer le service de l'icône puis s'arrêter.
    case startServiceAndExit
    /// Le service tourne déjà : ouvrir Admin (seconde ouverture) et s'arrêter.
    case openAdminAndExit
    /// Une autre icône tient le verrou : lui demander de partir, puis attendre.
    case replaceHolder
    /// Une autre icône du service tient déjà le verrou : s'arrêter.
    case exit
}

/// La décision, sans effet de bord (essayée par `--check-instance`).
/// - `isJob` : cette copie est-elle le service launchd de l'icône ?
/// - `hasPlist` : le service est-il installé (sinon : copie de développement) ?
/// - `jobRunning` : le service tourne-t-il (vu par `launchctl print`) ?
/// - `lockFree` : le verrou a-t-il pu être pris ?
/// - `holderIsJob` : quand le verrou est tenu, l'est-il par le service ?
func instanceRole(isJob: Bool, hasPlist: Bool, jobRunning: Bool, lockFree: Bool, holderIsJob: Bool) -> InstanceRole {
    if isJob {
        if lockFree { return .run }
        return holderIsJob ? .exit : .replaceHolder
    }
    if hasPlist { return jobRunning ? .openAdminAndExit : .startServiceAndExit }
    // Sans service installé (essais) : la première copie garde l'icône.
    return lockFree ? .run : .openAdminAndExit
}

/// Verrou de l'icône : `flock` sur un fichier qui contient le pid et
/// l'origine (« job » ou « app ») de la copie qui le tient.
final class InstanceLock {
    let url: URL
    private var fd: Int32 = -1

    init(url: URL) { self.url = url }

    /// Prend le verrou sans attendre ; vrai si pris.
    func tryLock(isJob: Bool) -> Bool {
        if fd < 0 {
            try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            fd = open(url.path, O_RDWR | O_CREAT | O_CLOEXEC | O_NOFOLLOW, 0o600)
            if fd < 0 { return true } // Pas de verrou possible : ne pas priver le Mac d'icône.
        }
        guard flock(fd, LOCK_EX | LOCK_NB) == 0 else { return false }
        let text = "\(getpid()) \(isJob ? "job" : "app")\n"
        ftruncate(fd, 0)
        _ = text.withCString { pwrite(fd, $0, strlen($0), 0) }
        return true
    }

    /// Attend le verrou jusqu'à `seconds`.
    func lock(isJob: Bool, within seconds: Double) -> Bool {
        let end = Date().addingTimeInterval(seconds)
        repeat {
            if tryLock(isJob: isJob) { return true }
            Thread.sleep(forTimeInterval: 0.1)
        } while Date() < end
        return false
    }

    /// Pid et origine de la copie qui tient le verrou (lus dans le fichier).
    func holder() -> (pid: pid_t, isJob: Bool)? {
        guard let text = try? String(contentsOf: url, encoding: .utf8) else { return nil }
        let parts = text.split(whereSeparator: { $0 == " " || $0 == "\n" })
        guard parts.count >= 2, let pid = pid_t(parts[0]), pid > 0 else { return nil }
        return (pid, parts[1] == "job")
    }

    func unlock() {
        if fd >= 0 { flock(fd, LOCK_UN); close(fd); fd = -1 }
    }
}

/// Cette copie est-elle lancée par le service launchd de l'icône ? launchd
/// donne le nom du service dans `XPC_SERVICE_NAME` ; LaunchServices y met
/// `application.<identifiant>.<nombres>`.
var launchedByMenubarJob: Bool {
    ProcessInfo.processInfo.environment["XPC_SERVICE_NAME"] == Labels.menubar
}

/// Le verrou, gardé ouvert toute la vie de l'icône.
let instanceLock = InstanceLock(url: Paths.station.appendingPathComponent("menubar.lock"))

/// Au lancement de l'icône : vrai si cette copie doit rester.
func claimSingleInstance() -> Bool {
    let isJob = launchedByMenubarJob
    let hasPlist = FileManager.default.fileExists(atPath: Paths.plist(Labels.menubar).path)
    let lockFree = instanceLock.tryLock(isJob: isJob)
    let holder = lockFree ? nil : instanceLock.holder()
    let running = !isJob && hasPlist && serviceState(Labels.menubar) == .running
    switch instanceRole(isJob: isJob, hasPlist: hasPlist, jobRunning: running, lockFree: lockFree, holderIsJob: holder?.isJob ?? false) {
    case .run:
        if isJob { endOlderCopies() }
        return true
    case .startServiceAndExit:
        // Une copie ouverte à la main qui tient le verrou (sans service) :
        // le service la remplacera.
        instanceLock.unlock()
        startService(Labels.menubar)
        return false
    case .openAdminAndExit:
        instanceLock.unlock()
        if let url = adminURL() { _ = run("/usr/bin/open", [url.absoluteString]) }
        return false
    case .replaceHolder:
        if let pid = holder?.pid, pid != getpid() { kill(pid, SIGTERM) }
        if instanceLock.lock(isJob: true, within: 5) { return true }
        if let pid = holder?.pid, pid != getpid() { kill(pid, SIGKILL) }
        guard instanceLock.lock(isJob: true, within: 2) else { return false }
        endOlderCopies()
        return true
    case .exit:
        return false
    }
}

/// Copies d'avant ce correctif, qui ne prennent pas le verrou : toute autre
/// icône (même exécutable, même identifiant) est arrêtée par le service.
/// Les fenêtres de `kybernow-ui`, du même paquet, ont un autre exécutable.
func endOlderCopies() {
    let me = getpid()
    // Hors d'un paquet (essai de l'exécutable seul) : rien à arrêter.
    guard let id = Bundle.main.bundleIdentifier, let name = Bundle.main.executableURL?.lastPathComponent else { return }
    for app in NSRunningApplication.runningApplications(withBundleIdentifier: id)
    where app.processIdentifier != me && app.executableURL?.lastPathComponent == name {
        if !app.terminate() { kill(app.processIdentifier, SIGTERM) }
    }
}

/// `--check-instance` : la décision dans tous les cas, et le verrou entre
/// deux ouvertures du même fichier (deux copies de l'icône).
func checkInstance() -> Bool {
    var ok = true
    func expect(_ got: InstanceRole, _ want: InstanceRole, _ what: String) {
        if got != want { print("instance: \(what) : \(got), attendu \(want)"); ok = false }
    }
    // Le service garde l'icône s'il a le verrou.
    expect(instanceRole(isJob: true, hasPlist: true, jobRunning: true, lockFree: true, holderIsJob: false), .run, "service seul")
    // Le service trouve une copie ouverte à la main : il la remplace.
    expect(instanceRole(isJob: true, hasPlist: true, jobRunning: true, lockFree: false, holderIsJob: false), .replaceHolder, "service après Spotlight")
    expect(instanceRole(isJob: true, hasPlist: true, jobRunning: true, lockFree: false, holderIsJob: true), .exit, "service en double")
    // Copie ouverte à la main (Spotlight, Finder) : jamais gardée quand le service existe.
    expect(instanceRole(isJob: false, hasPlist: true, jobRunning: false, lockFree: true, holderIsJob: false), .startServiceAndExit, "Spotlight, service arrêté")
    expect(instanceRole(isJob: false, hasPlist: true, jobRunning: true, lockFree: false, holderIsJob: true), .openAdminAndExit, "Spotlight, service en marche")
    expect(instanceRole(isJob: false, hasPlist: true, jobRunning: true, lockFree: true, holderIsJob: false), .openAdminAndExit, "Spotlight, service en marche sans verrou")
    // Sans service (essais) : la première copie seulement.
    expect(instanceRole(isJob: false, hasPlist: false, jobRunning: false, lockFree: true, holderIsJob: false), .run, "essai, première copie")
    expect(instanceRole(isJob: false, hasPlist: false, jobRunning: false, lockFree: false, holderIsJob: false), .openAdminAndExit, "essai, seconde copie")

    let dir = FileManager.default.temporaryDirectory.appendingPathComponent("kybernow-instance-\(getpid())")
    defer { try? FileManager.default.removeItem(at: dir) }
    let file = dir.appendingPathComponent("menubar.lock")
    let first = InstanceLock(url: file), second = InstanceLock(url: file)
    if !first.tryLock(isJob: false) { print("instance: premier verrou refusé"); ok = false }
    if second.tryLock(isJob: true) { print("instance: second verrou accordé alors que le premier le tient"); ok = false }
    if let holder = second.holder() {
        if holder.pid != getpid() || holder.isJob { print("instance: titulaire lu \(holder)"); ok = false }
    } else { print("instance: titulaire illisible"); ok = false }
    first.unlock()
    if !second.tryLock(isJob: true) { print("instance: verrou rendu mais pas repris"); ok = false }
    if second.holder()?.isJob != true { print("instance: le titulaire devrait être le service"); ok = false }
    second.unlock()
    return ok
}
