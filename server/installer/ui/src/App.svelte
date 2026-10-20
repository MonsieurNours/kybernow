<script lang="ts">
  import { fade, fly } from 'svelte/transition'
  import Rail from './lib/Rail.svelte'
  import TitleBar from './lib/TitleBar.svelte'
  import { app, reducedMotion, startBackend } from './lib/state.svelte'
  import About from './screens/About.svelte'
  import Account from './screens/Account.svelte'
  import Done from './screens/Done.svelte'
  import Install from './screens/Install.svelte'
  import Options from './screens/Options.svelte'
  import Reinstall from './screens/Reinstall.svelte'
  import Welcome from './screens/Welcome.svelte'

  startBackend()

  const screens = { welcome: Welcome, reinstall: Reinstall, account: Account, options: Options, install: Install, done: Done, about: About }
  const Screen = $derived(screens[app.screen])
  const inMotion = reducedMotion ? { duration: 0 } : { x: 12, duration: 420, delay: 90 }
  const outMotion = reducedMotion ? { duration: 0 } : { duration: 90 }
</script>

<div class="window">
  <Rail />
  <section>
    <TitleBar />
    <div class="stage">
      {#key app.screen}
        <div class="screen" in:fly={inMotion} out:fade={outMotion}>
          <Screen />
        </div>
      {/key}
    </div>
  </section>
</div>

<style>
  .window {
    width: 100vw;
    height: 100vh;
    display: flex;
    overflow: hidden;
    background: var(--ink-0);
  }

  section {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .stage {
    flex: 1;
    min-height: 0;
    display: grid;
  }

  .screen {
    grid-area: 1 / 1;
    min-height: 0;
    padding: 12px 56px 0;
    display: flex;
    flex-direction: column;
  }
</style>
