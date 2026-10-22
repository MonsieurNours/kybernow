package fr.leslignesclaires.kybernow.model

import java.io.ByteArrayOutputStream
import java.net.DatagramPacket
import java.net.Inet4Address
import java.net.InetAddress
import java.net.MulticastSocket
import java.net.NetworkInterface
import java.net.SocketTimeoutException

/**
 * Adresse IPv4 d'un nom `.local` demandée directement en mDNS (enregistrement
 * A, réponse en unicast demandée) : dernier recours de la découverte quand
 * NsdManager et JmDNS ne rendent pas d'IPv4. Admin s'annonce sous le nom de
 * l'ordinateur en minuscules (`pc-de-matthieu.local`).
 */
object MdnsLookup {
    private val GROUP: InetAddress = InetAddress.getByName("224.0.0.251")
    private const val PORT = 5353

    fun ipv4(host: String, local: InetAddress?, timeoutMs: Int = 2000): String? {
        val name = host.trimEnd('.').lowercase().let { if (it.endsWith(".local")) it else "$it.local" }
        val query = query(name)
        MulticastSocket(0).use { s ->
            if (local != null) runCatching { NetworkInterface.getByInetAddress(local)?.let { s.networkInterface = it } }
            s.timeToLive = 255
            s.soTimeout = 400
            val deadline = System.currentTimeMillis() + timeoutMs
            var sent = 0
            val buf = ByteArray(1500)
            while (System.currentTimeMillis() < deadline) {
                if (sent < 3) {
                    s.send(DatagramPacket(query, query.size, GROUP, PORT))
                    sent++
                }
                try {
                    val p = DatagramPacket(buf, buf.size)
                    s.receive(p)
                    parseA(buf.copyOf(p.length), name)?.let { return it }
                } catch (_: SocketTimeoutException) {
                }
            }
        }
        return null
    }

    private fun query(name: String): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(byteArrayOf(0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0))
        for (label in name.split('.')) {
            val b = label.toByteArray()
            out.write(b.size)
            out.write(b)
        }
        out.write(0)
        out.write(byteArrayOf(0, 1, 0x80.toByte(), 1)) // type A, classe IN avec réponse unicast demandée
        return out.toByteArray()
    }

    /** Premier enregistrement A pour `name` dans une réponse (réponses et données additionnelles). */
    internal fun parseA(m: ByteArray, name: String): String? {
        if (m.size < 12) return null
        fun u16(i: Int) = ((m[i].toInt() and 0xFF) shl 8) or (m[i + 1].toInt() and 0xFF)
        fun readName(start: Int): Pair<String, Int> {
            val labels = mutableListOf<String>()
            var i = start
            var end = -1
            var jumps = 0
            while (i < m.size) {
                val len = m[i].toInt() and 0xFF
                when {
                    len == 0 -> { if (end < 0) end = i + 1; break }
                    len and 0xC0 == 0xC0 -> {
                        if (end < 0) end = i + 2
                        i = ((len and 0x3F) shl 8) or (m[i + 1].toInt() and 0xFF)
                        if (++jumps > 20) break
                    }
                    else -> {
                        labels += String(m, i + 1, len)
                        i += 1 + len
                    }
                }
            }
            return labels.joinToString(".").lowercase() to (if (end < 0) m.size else end)
        }
        val qd = u16(4)
        val records = u16(6) + u16(8) + u16(10)
        var i = 12
        repeat(qd) { i = readName(i).second + 4 }
        repeat(records) {
            if (i >= m.size) return null
            val (n, after) = readName(i)
            if (after + 10 > m.size) return null
            val type = u16(after)
            val len = u16(after + 8)
            val data = after + 10
            if (type == 1 && len == 4 && n == name && data + 4 <= m.size) {
                return (Inet4Address.getByAddress(m.copyOfRange(data, data + 4)) as InetAddress).hostAddress
            }
            i = data + len
        }
        return null
    }
}
