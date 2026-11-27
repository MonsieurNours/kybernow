// Commun aux pages de l'icône et de l'agent de console (panneau, lanceur du
// prêt), servi en /common.js par chacune avant son propre script.
// Un message à l'icône ou à l'agent (`window.ipc.postMessage`).
const send = (cmd, extra) => window.ipc && window.ipc.postMessage(JSON.stringify({ cmd, ...extra }))
// Un texte à mettre dans du HTML.
const esc = (t) => String(t ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c])
