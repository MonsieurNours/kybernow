package fr.leslignesclaires.kybernow.model

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress

/** Réveil de l'ordinateur par le réseau : paquet magique en diffusion (port 9) et vers son adresse. */
object WakeOnLan {
    suspend fun send(mac: String, address: String) = withContext(Dispatchers.IO) {
        val bytes = mac.split(':', '-').mapNotNull { it.toIntOrNull(16)?.toByte() }
        if (bytes.size != 6) return@withContext
        val packet = ByteArray(6) { 0xFF.toByte() } + (0 until 16).flatMap { bytes }.toByteArray()
        runCatching {
            DatagramSocket().use { s ->
                s.broadcast = true
                for (target in listOf("255.255.255.255", address)) {
                    runCatching { s.send(DatagramPacket(packet, packet.size, InetAddress.getByName(target), 9)) }
                }
            }
        }
    }
}
