package fr.leslignesclaires.kybernow

import fr.leslignesclaires.kybernow.session.StationSystem
import org.junit.Assert.assertEquals
import org.junit.Test

/** L'avertissement du prêt suit le système de la Station (docs/link-ui.md, « Prêt de l'ordinateur »). */
class StationSystemTest {
    @Test
    fun theWarningFollowsTheStationsSystem() {
        assertEquals(StationSystem.WINDOWS, StationSystem.of("Windows 11 Pro 24H2", mac = false))
        // Mac : la règle du cœur (`station_is_mac`), donnée par l'app.
        assertEquals(StationSystem.MAC, StationSystem.of("macOS 26.1", mac = true))
        assertEquals(StationSystem.OTHER, StationSystem.of("Ubuntu 24.04", mac = false))
        assertEquals(StationSystem.OTHER, StationSystem.of(null, mac = false))
    }
}
