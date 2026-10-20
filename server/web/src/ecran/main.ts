import { mount } from 'svelte'
import '../app.css'
import Ecran from './Ecran.svelte'

const app = mount(Ecran, {
  target: document.getElementById('app')!,
})

export default app
