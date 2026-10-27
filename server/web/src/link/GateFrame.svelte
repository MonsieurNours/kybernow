<!--
  Cadre des écrans d'entrée du client web (mot de passe d'Admin, « Rejoindre
  un partage ») : la marque, puis un formulaire centré. Le contenu fournit
  `.head` (nom de l'ordinateur, titre, phrase), un `form` et une aide.
-->
<script lang="ts">
  import type { Snippet } from 'svelte'
  import Mark from '../lib/Mark.svelte'

  let { children }: { children: Snippet } = $props()
</script>

<div class="gate">
  <header>
    <Mark product="link" size={26} />
  </header>

  <main>{@render children()}</main>
</div>

<style>
  .gate {
    min-height: 100vh;
    min-height: 100dvh;
    display: flex;
    flex-direction: column;
    background:
      radial-gradient(60% 50% at 80% 10%, rgb(109 91 255 / 0.16), transparent 70%),
      var(--ink-0);
  }

  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 28px max(24px, 5vw);
  }

  main {
    flex: 1;
    width: min(440px, calc(100% - 48px));
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 26px;
    padding-bottom: 12vh;
  }

  .gate :global(.head) {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .gate :global(.host) {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-2);
  }

  .gate :global(.dot) {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--ok);
  }

  .gate :global(h1) {
    font-size: clamp(32px, 6vw, 44px);
    line-height: 1.02;
    font-weight: 650;
    letter-spacing: -0.035em;
    text-wrap: balance;
  }

  .gate :global(.head p) {
    color: var(--muted);
  }

  .gate :global(form) {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .gate :global(input) {
    height: 50px;
    padding: 0 16px;
    font-size: 16px;
    border-radius: 12px;
  }

  .gate :global(.primary) {
    height: 52px;
    font-size: 16px;
  }

  .gate :global(.error) {
    font-size: 13.5px;
    color: var(--bad-text);
  }

  .gate :global(.hint) {
    font-size: 12.5px;
    color: var(--faint);
  }
</style>
