<script lang="ts">
  import { fade, fly } from 'svelte/transition'
  import Rail from './lib/Rail.svelte'
  import SignalBackground from './lib/SignalBackground.svelte'
  import TitleBar from './lib/TitleBar.svelte'
  import { app, reducedMotion, startBackend } from './lib/state.svelte'
  import About from './screens/About.svelte'
  import Account from './screens/Account.svelte'
  import Checks from './screens/Checks.svelte'
  import Done from './screens/Done.svelte'
  import Install from './screens/Install.svelte'
  import Options from './screens/Options.svelte'
  import Welcome from './screens/Welcome.svelte'

  startBackend()

  const screens = { welcome: Welcome, checks: Checks, account: Account, options: Options, install: Install, done: Done, about: About }
  const Screen = $derived(screens[app.screen])
  const speed = $derived(app.installing ? 1.8 : 1)
  const inMotion = reducedMotion ? { duration: 0 } : { y: 16, duration: 420, delay: 90 }
  const outMotion = reducedMotion ? { duration: 0 } : { duration: 90 }
</script>

<div class="window">
  <TitleBar />
  <div class="body">
    <Rail />
    <main>
      <SignalBackground {speed} />
      {#key app.screen}
        <div class="screen" in:fly={inMotion} out:fade={outMotion}>
          <Screen />
        </div>
      {/key}
    </main>
  </div>
</div>

<style>
  .window {
    width: 100vw;
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .body {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  main {
    position: relative;
    flex: 1;
    min-width: 0;
    display: grid;
  }

  .screen {
    grid-area: 1 / 1;
    position: relative;
    z-index: 1;
    min-height: 0;
    padding: 40px 56px 28px;
    display: flex;
    flex-direction: column;
  }
</style>
