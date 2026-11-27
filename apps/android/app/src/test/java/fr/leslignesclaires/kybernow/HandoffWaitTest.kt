package fr.leslignesclaires.kybernow

import android.app.Application
import fr.leslignesclaires.kybernow.playstation.HandoffWait
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.util.Collections
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/** « Recevoir d'un autre appareil » (AND-09) : la poignée n'est jamais libérée pendant l'appel bloquant. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class HandoffWaitTest {
    @Test
    fun stopWaitsForTheBlockingCall() = runBlocking {
        val log = Collections.synchronizedList(mutableListOf<String>())
        val inside = CountDownLatch(1)
        val busy = AtomicBoolean(false)
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        val job = HandoffWait.launch(
            scope, null,
            start = { log += "start"; 7L },
            next = { h ->
                busy.set(true); inside.countDown()
                Thread.sleep(150)
                busy.set(false); log += "next $h"; null
            },
            stop = { h -> assertFalse("libérée pendant l'appel", busy.get()); log += "stop $h" },
            onStart = {}, onEvent = { false },
        )
        assertTrue(inside.await(2, TimeUnit.SECONDS))
        // L'écran se ferme pendant l'attente.
        job.cancel()
        job.join()
        assertEquals(listOf("start", "next 7", "stop 7"), log.toList())
    }

    @Test
    fun nextWaitStartsAfterThePrevious() = runBlocking {
        val log = Collections.synchronizedList(mutableListOf<String>())
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        val first = HandoffWait.launch(
            scope, null, start = { 1L }, next = { Thread.sleep(80); null }, stop = { log += "stop $it" }, onStart = {}, onEvent = { false },
        )
        Thread.sleep(20)
        first.cancel()
        val second = HandoffWait.launch(
            scope, first, start = { log += "start 2"; 2L }, next = { """{"event":"expired"}""" }, stop = { log += "stop $it" },
            onStart = {}, onEvent = { it.optString("event") == "expired" },
        )
        second.join()
        assertEquals(listOf("stop 1", "start 2", "stop 2"), log.toList())
    }

    @Test
    fun unreadableEventsAreSkipped() = runBlocking {
        val events = ArrayDeque(listOf("illisible", """{"event":"new_code","code":"123456"}""", """{"event":"expired"}"""))
        val seen = mutableListOf<String>()
        HandoffWait.launch(
            CoroutineScope(Dispatchers.Default), null, start = { 3L }, next = { events.removeFirstOrNull() }, stop = {},
            onStart = {}, onEvent = { seen += it.optString("event"); it.optString("event") == "expired" },
        ).join()
        assertEquals(listOf("new_code", "expired"), seen)
    }

    @Test
    fun noHandleNoLoop() = runBlocking {
        var stopped = false
        HandoffWait.launch(CoroutineScope(Dispatchers.Default), null, start = { 0L }, next = { error("jamais") }, stop = { stopped = true }, onStart = {}, onEvent = { true }).join()
        assertFalse(stopped)
    }
}
