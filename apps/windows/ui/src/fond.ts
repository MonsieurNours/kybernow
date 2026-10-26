import { mount } from 'svelte'
import './app.css'
import Fond from './Fond.svelte'

export default mount(Fond, { target: document.getElementById('app')! })
