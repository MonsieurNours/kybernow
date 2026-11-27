// Le menu contextuel de WebView2 (« Actualiser », « Inspecter »…) n'a rien à
// faire dans l'app : un « Actualiser » sur la barre rechargeait la page et
// perdait son état (feuille ouverte, réglages en cours). Gardé dans les champs
// de saisie (couper, copier, coller). Importé par les quatre fenêtres.
const editable = (t: EventTarget | null) =>
  t instanceof HTMLElement && (t.isContentEditable || t.closest('input, textarea, [contenteditable="true"]') !== null)

window.addEventListener('contextmenu', (e) => {
  if (!editable(e.target)) e.preventDefault()
})
