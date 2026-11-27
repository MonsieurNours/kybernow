<!--
  Le fond d'une feuille (docs/link-ui.md) et ce qui va avec, pour toutes les
  feuilles et confirmations de l'app : clic dehors, Échap (la feuille du
  dessus seulement), tout le reste de la page inerte tant qu'elle est ouverte
  (ni clic, ni Tab, ni lecteur d'écran derrière), focus posé dans la feuille à
  l'ouverture et rendu à la fermeture. La feuille elle-même (son cadre, sa
  largeur, `role="dialog"`) reste dans chaque composant.
-->
<script module lang="ts">
  /** Les feuilles ouvertes, la dernière au-dessus : Échap ne ferme qu'elle. */
  const stack: symbol[] = []
</script>

<script lang="ts">
  import { onMount, type Snippet } from 'svelte'

  let {
    onclose,
    outside = true,
    escape = true,
    z = 30,
    fixed = false,
    shade = 'rgb(5 7 10 / 0.7)',
    animate = true,
    children,
  }: {
    /** Fermer (clic dehors, Échap). */
    onclose?: () => void
    /** Un clic sur le fond ferme. */
    outside?: boolean
    /** Échap ferme. */
    escape?: boolean
    z?: number
    /** Sur toute la fenêtre (confirmations), sinon sur la zone de l'accueil. */
    fixed?: boolean
    shade?: string
    animate?: boolean
    children: Snippet
  } = $props()

  const me = Symbol('feuille')
  let veil: HTMLDivElement

  onMount(() => {
    stack.push(me)
    const before = document.activeElement instanceof HTMLElement ? document.activeElement : null
    // Tout ce qui n'est pas cette feuille (ni ses parents) devient inerte.
    const made: Element[] = []
    for (let node: Element | null = veil; node && node !== document.body; node = node.parentElement) {
      for (const sibling of Array.from(node.parentElement?.children ?? [])) {
        if (sibling !== node && !sibling.hasAttribute('inert') && sibling.tagName !== 'SCRIPT') {
          sibling.setAttribute('inert', '')
          made.push(sibling)
        }
      }
    }
    if (!veil.contains(document.activeElement)) {
      veil.querySelector<HTMLElement>('[autofocus], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), button:not([disabled]), [href], [tabindex]:not([tabindex="-1"])')?.focus()
    }
    const key = (e: KeyboardEvent) => {
      // Une touche déjà prise (un champ qui annule sa saisie) ne ferme pas.
      if (e.key !== 'Escape' || e.defaultPrevented || stack.at(-1) !== me || !escape || !onclose) return
      e.preventDefault()
      onclose()
    }
    window.addEventListener('keydown', key)
    return () => {
      window.removeEventListener('keydown', key)
      made.forEach((m) => m.removeAttribute('inert'))
      stack.splice(stack.indexOf(me), 1)
      if (before?.isConnected) before.focus()
    }
  })
</script>

<div
  class="veil"
  class:fixed
  class:animate
  style:z-index={z}
  style:background={shade}
  role="presentation"
  bind:this={veil}
  onclick={(e) => e.target === e.currentTarget && outside && onclose?.()}
>
  {@render children()}
</div>

<style>
  .veil {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 56px 24px 24px;
  }
  .veil.fixed {
    position: fixed;
  }
  .veil.animate {
    animation: sheet-fade 0.25s var(--ease-out);
  }
</style>
