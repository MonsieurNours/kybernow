package fr.leslignesclaires.kybernow.model

import fr.leslignesclaires.kybernow.core.AccountComputer
import fr.leslignesclaires.kybernow.core.AccountMerge
import fr.leslignesclaires.kybernow.core.HostRelay
import java.util.UUID

/**
 * Les ordinateurs des comptes KyberNow à l'accueil (docs/comptes.md, « Mes ordinateurs, partout ») : gardés avec les
 * ordinateurs de l'appareil, pour avoir les mêmes cartes, les mêmes réglages, la même dernière session. Un ordinateur à
 * la fois appairé et sur un compte n'apparaît qu'une fois ([AccountMerge] du cœur) : l'appairé, qui prend la pastille du
 * compte ([Host.account]) ; un ordinateur de deux comptes, une fois.
 */
object AccountHosts {
    /**
     * La liste de l'accueil d'après la fusion du cœur ([merge]) : un ordinateur venu d'un compte qui n'y est plus part ;
     * celui qui y est encore garde sa carte (identifiant, réglages, dernière session, adresse vue sur le réseau) et suit
     * le compte (nom, empreinte d'Admin, relais) ; les nouveaux arrivent au bout. Un ordinateur appairé prend la pastille
     * du compte s'il y est, la perd sinon. Rend [hosts] tel quel quand rien ne change.
     */
    fun sync(hosts: List<Host>, merge: AccountMerge): List<Host> {
        val added = merge.added.toMutableList()
        val out = mutableListOf<Host>()
        for (h in hosts) {
            if (h.fromAccount) {
                // La même Station chez le même relais, même si elle vient maintenant d'un autre compte.
                val c = added.firstOrNull { it.station == h.account?.station && it.relay == h.relay?.server } ?: continue
                added.remove(c)
                out += follow(h, c)
            } else {
                val acc = merge.linked[h.id]?.let { HostAccount(it.account, it.station) }
                out += if (acc != h.account) h.copy(account = acc) else h
            }
        }
        added.forEach { c ->
            out += follow(Host(id = "compte-" + UUID.randomUUID().toString(), name = "", address = "", port = 0, fingerprint = null, user = ""), c)
        }
        return if (out == hosts) hosts else out
    }

    /** L'ordinateur tel que le compte le donne ; l'empreinte d'Admin déjà connue reste si le relais ne la redonne pas. */
    private fun follow(h: Host, c: AccountComputer): Host = h.copy(
        name = c.name.ifBlank { h.name.ifBlank { c.station } },
        fingerprint = c.adminFp ?: h.fingerprint,
        relay = HostRelay(c.relay, c.station, remoteAccess = true),
        account = HostAccount(c.account, c.station),
        paired = false,
    )
}
