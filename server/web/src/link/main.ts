import { mount } from 'svelte'
import '../app.css'
import App from './App.svelte'
import { probeAtLoad } from './pyrowave/state.svelte'

// PyroWave : sonde au chargement (WebGPU, décodage d'une image de test), en fond.
probeAtLoad()

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
