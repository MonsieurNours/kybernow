import { mount } from 'svelte'
import '../app.css'
import App from './App.svelte'

// PyroWave : la sonde (WebGPU, décodage d'une image de test) attend qu'une Station le propose
// (`probeAtLoad`, accueil et session de l'hôte).

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
