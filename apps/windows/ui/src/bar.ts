import { mount } from 'svelte'
import './app.css'
import Bar from './Bar.svelte'

export default mount(Bar, { target: document.getElementById('app')! })
