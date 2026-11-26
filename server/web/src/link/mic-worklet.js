// @ts-nocheck : portée globale d'AudioWorkletGlobalScope, hors des types de la page.
// Micro du client web (micro.svelte.ts) : le nœud qui reçoit le son du micro
// dans l'AudioContext et le rend à la page par trames de 10 ms, mono (la
// première voie). Fichier JavaScript simple : chargé tel quel par
// audioWorklet.addModule, hors de la compilation de la page.

class KyberNowMic extends AudioWorkletProcessor {
  constructor(options) {
    super()
    // 10 ms à la fréquence du contexte (480 à 48 kHz).
    this.size = Math.round(((options && options.processorOptions && options.processorOptions.rate) || sampleRate) / 100)
    this.buf = new Float32Array(this.size)
    this.fill = 0
  }

  process(inputs) {
    const ch = inputs[0] && inputs[0][0]
    if (!ch) return true
    let i = 0
    while (i < ch.length) {
      const n = Math.min(ch.length - i, this.size - this.fill)
      this.buf.set(ch.subarray(i, i + n), this.fill)
      this.fill += n
      i += n
      if (this.fill === this.size) {
        this.port.postMessage(this.buf, [this.buf.buffer])
        this.buf = new Float32Array(this.size)
        this.fill = 0
      }
    }
    return true
  }
}

registerProcessor('kybernow-mic', KyberNowMic)
