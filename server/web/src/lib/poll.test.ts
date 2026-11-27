import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Generation, poll } from './poll'

describe('poll', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => vi.useRealTimers())

  it('saute un tour tant que le précédent n’a pas répondu', async () => {
    let calls = 0
    let release: () => void = () => {}
    const stop = poll(
      () => {
        calls++
        return new Promise<void>((ok) => (release = ok))
      },
      1000,
    )
    expect(calls).toBe(1)
    await vi.advanceTimersByTimeAsync(3000)
    expect(calls).toBe(1)
    release()
    await vi.advanceTimersByTimeAsync(1000)
    expect(calls).toBe(2)
    stop()
    release()
    await vi.advanceTimersByTimeAsync(5000)
    expect(calls).toBe(2)
  })

  it('ne relève rien onglet masqué, et relève au retour', async () => {
    let hidden = true
    const doc = new EventTarget() as EventTarget & { hidden: boolean }
    Object.defineProperty(doc, 'hidden', { get: () => hidden })
    vi.stubGlobal('document', doc)
    let calls = 0
    const stop = poll(() => void calls++, 1000)
    await vi.advanceTimersByTimeAsync(5000)
    expect(calls).toBe(0)
    hidden = false
    doc.dispatchEvent(new Event('visibilitychange'))
    await vi.advanceTimersByTimeAsync(0)
    expect(calls).toBe(1)
    stop()
    vi.unstubAllGlobals()
  })
})

describe('Generation', () => {
  it('rend périmé un relevé parti avant une écriture locale', () => {
    const g = new Generation()
    const at = g.current()
    expect(g.fresh(at)).toBe(true)
    g.bump()
    expect(g.fresh(at)).toBe(false)
  })
})
