// « Télécharger le journal » du client web : en-tête, messages gardés, valeurs de la console. `npm test`.

import { describe, expect, it } from 'vitest'

import { describe as device, journalText, keep, show, utc } from './journal'

const CHROME = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36'
const SAFARI = 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1'
const EDGE = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.2903.70'

describe('journal du client web', () => {
  it("l'en-tête que lit la page du relais, puis les messages", () => {
    const now = new Date(Date.UTC(2026, 10, 27, 12, 0, 0))
    expect(utc(now)).toBe('2026-11-27 12:00:00.000 UTC')
    const text = journalText('0.11.8', device(CHROME, 'MacIntel'), CHROME, now, false, ['a', 'b'], 0)
    expect(text).toBe(`KyberNow Link 0.11.8 (client web)\nAppareil : Chrome 131.0.0.0 · macOS\nJournal écrit le 2026-11-27 12:00:00.000 UTC\nAgent : ${CHROME}\n\na\nb\n`)
    expect(journalText('0.11.8', device(CHROME, ''), CHROME, now, true, [], 3)).toContain('Device: Chrome 131.0.0.0 · macOS\nLog written on 2026-11-27 12:00:00.000 UTC\nUser agent:')
    expect(journalText('0.11.8', device(CHROME, ''), CHROME, now, true, [], 3)).toContain('[… 3 older messages removed]')
  })

  it('navigateurs et systèmes reconnus', () => {
    expect(device(SAFARI, 'iPhone')).toEqual({ browser: 'Safari 18.1', system: 'iOS' })
    expect(device(EDGE, 'Win32')).toEqual({ browser: 'Edge 131.0.2903.70', system: 'Windows' })
    expect(device('Inconnu/1', 'Plan9')).toEqual({ browser: 'Inconnu/1', system: 'Plan9' })
  })

  it('valeurs de la console en texte', () => {
    expect(show('x')).toBe('x')
    expect(show({ a: 1 })).toBe('{"a":1}')
    expect(show(undefined)).toBe('undefined')
    const cyclic: Record<string, unknown> = {}
    cyclic.self = cyclic
    expect(show(cyclic)).toBe('[object Object]')
    expect(show(new Error('boum'))).toContain('boum')
  })

  it('au-delà de la limite, les plus anciens messages partent', () => {
    for (let i = 0; i < 10; i++) keep(`ligne ${i}`, 30)
    const text = journalText('0', { browser: 'b', system: 's' }, 'ua', new Date(0), false)
    expect(text).toContain('ligne 9\n')
    expect(text).not.toContain('ligne 0\n')
    expect(text).toMatch(/\[… \d+ messages plus anciens retirés\]/)
  })
})
