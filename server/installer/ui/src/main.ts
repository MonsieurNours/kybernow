import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { isNative } from './lib/backend'

// Dans l'installateur, pas de menu contextuel ni de zoom du navigateur.
if (isNative) {
  window.addEventListener('contextmenu', (e) => e.preventDefault())
  window.addEventListener('wheel', (e) => e.ctrlKey && e.preventDefault(), { passive: false })
}

export default mount(App, { target: document.getElementById('app')! })
