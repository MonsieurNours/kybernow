import { beforeAll, describe, expect, it, vi } from 'vitest'

// `lend.ts` passe par `device.ts`, qui lit `document` et `navigator` au chargement.
const store = new Map<string, string>()
let lend: typeof import('./lend')
beforeAll(async () => {
  vi.stubGlobal('document', { documentElement: {}, addEventListener() {} })
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, v),
    removeItem: (k: string) => void store.delete(k),
  })
  lend = await import('./lend')
})

describe('ce que le navigateur retient d’un prêt (WEB-15)', () => {
  it('le code de prêt n’est pas écrit en clair', () => {
    lend.markActive({ code: '482913' })
    const raw = [...store.values()].join(' ')
    expect(raw).not.toContain('482913')
    expect(lend.isActive({ code: '482913' })).toBe(true)
    expect(lend.isActive({ code: '482914' })).toBe(false)
  })

  it('un prêt quitté depuis plus de 15 min est effacé', () => {
    store.set('kybernow.lend.active', JSON.stringify({ key: lend.lendKey({ code: '1' }), at: Date.now() - lend.COME_BACK_MS - 1 }))
    expect(lend.isActive({ code: '1' })).toBe(false)
    expect(store.has('kybernow.lend.active')).toBe(false)
  })

  it('un compte inutilisé depuis 90 jours est oublié, mot de passe compris', () => {
    const now = Date.now()
    store.set(
      'kybernow.lend.accounts',
      JSON.stringify({ vieux: { password: 'x', used: now - lend.FORGET_AFTER_MS - 1 }, recent: { password: 'y', used: now }, ancien: { host: 'Léa' } }),
    )
    const all = lend.readAccounts(now)
    expect(Object.keys(all).sort()).toEqual(['ancien', 'recent'])
    expect(all.ancien.used).toBe(now)
    expect(store.get('kybernow.lend.accounts')).not.toContain('"x"')
  })
})
