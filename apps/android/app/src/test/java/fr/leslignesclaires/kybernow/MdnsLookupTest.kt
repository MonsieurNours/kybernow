package fr.leslignesclaires.kybernow

import fr.leslignesclaires.kybernow.model.MdnsLookup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class MdnsLookupTest {
    private fun name(n: String) = n.split('.').flatMap { listOf(it.length.toByte()) + it.toByteArray().toList() } + listOf(0.toByte())

    /** Réponse : un AAAA puis un A dont le nom renvoie (compression) au premier. */
    private fun response(): ByteArray {
        val b = mutableListOf<Byte>()
        b += listOf(0, 0, 0x84, 0, 0, 0, 0, 2, 0, 0, 0, 0).map { it.toByte() }
        val first = b.size
        b += name("pc-de-matthieu.local")
        b += listOf(0, 28, 0x80, 1, 0, 0, 0, 120, 0, 16).map { it.toByte() } + List(16) { 0xFE.toByte() }
        b += listOf(0xC0.toByte(), first.toByte())
        b += listOf(0, 1, 0x80, 1, 0, 0, 0, 120, 0, 4, 192, 168, 1, 42).map { it.toByte() }
        return b.toByteArray()
    }

    @Test
    fun ipv4AfterIpv6WithCompressedName() = assertEquals("192.168.1.42", MdnsLookup.parseA(response(), "pc-de-matthieu.local"))

    @Test
    fun otherName() = assertNull(MdnsLookup.parseA(response(), "pc-atelier.local"))
}
