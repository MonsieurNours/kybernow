import { mount } from 'svelte'
import '@fontsource-variable/bricolage-grotesque/wght.css'
import '@fontsource/ibm-plex-sans/400.css'
import '@fontsource/ibm-plex-sans/500.css'
import '@fontsource/ibm-plex-sans/600.css'
import '@fontsource/ibm-plex-mono/400.css'
import '@fontsource/ibm-plex-mono/500.css'
import './app.css'
import App from './App.svelte'
import { isNative } from './lib/backend'

// Dans l'installateur, pas de menu contextuel ni de zoom du navigateur.
if (isNative) {
  window.addEventListener('contextmenu', (e) => e.preventDefault())
  window.addEventListener('wheel', (e) => e.ctrlKey && e.preventDefault(), { passive: false })
}

export default mount(App, { target: document.getElementById('app')! })
