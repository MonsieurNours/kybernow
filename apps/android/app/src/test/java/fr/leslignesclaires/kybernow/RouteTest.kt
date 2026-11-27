package fr.leslignesclaires.kybernow

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** L'écran gardé quand l'activité est recréée (AND-24). */
class RouteTest {
    private fun roundTrip(r: Route) = assertEquals(r, Route.restore(r.saved()))

    @Test
    fun screensComeBack() {
        listOf(
            Route.Home, Route.AppSettings, Route.Gamepads, Route.HostSettings("pc-1"), Route.ConsoleSettings("ps5"),
            Route.Join(), Route.Welcome(2, 1), Route.Add(pairConsole = "ps5"), Route.Add(),
            Route.AccountCode("share.kybernow.app", Route.AppSettings), Route.AccountRelay(true, Route.AccountCode("r", Route.Home)),
        ).forEach(::roundTrip)
    }

    @Test
    fun unknownIsIgnored() {
        assertNull(Route.restore(null))
        assertNull(Route.restore("autre"))
        assertNull(Route.restore("host"))
    }
}
