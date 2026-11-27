// Le temps et les avis de l'enregistrement (mêmes règles que le cœur). `npm test`.

import { beforeAll, describe, expect, it } from 'vitest'

import { elapsedNow, fileDuration, fileSize, readRecording, recordingClock, recordingNotice, recordingTooltip } from './recording'
import { setLangChoice } from './i18n/index.svelte'

const idle = readRecording({ allowed: true, active: false, last_end: null })!
const on = readRecording({ allowed: true, active: true, elapsed: 0, started_at: 5 })!
const saved = readRecording({ allowed: true, active: false, last_end: { reason: 'stopped', at: 1 } })!
const full = readRecording({ allowed: true, active: false, last_end: { reason: 'write', at: 1 } })!
const ended = readRecording({ allowed: true, active: false, last_end: { reason: 'ended', at: 1 } })!

describe("enregistrement de la session", () => {
  beforeAll(() => setLangChoice('fr'))

  it('compte le temps comme le cœur', () => {
    expect(recordingClock(0)).toBe('00:00')
    expect(recordingClock(134)).toBe('02:14')
    expect(recordingClock(3599)).toBe('59:59')
    expect(recordingClock(3734)).toBe('1:02:14')
    const s = readRecording({ allowed: true, active: true, elapsed: 130, started_at: 5 })
    expect(elapsedNow(s, 1000, 5500)).toBe(134)
    expect(elapsedNow(idle, 0, 99_000)).toBe(0)
  })

  it("ne montre rien sans « recording »", () => {
    expect(readRecording(undefined)).toBeNull()
    expect(readRecording(3)).toBeNull()
    expect(recordingNotice(undefined, null, 'host')).toBeNull()
  })

  it("donne les avis de l'hôte", () => {
    expect(recordingNotice(idle, on, 'host')).toBe("Enregistrement lancé sur l'ordinateur.")
    expect(recordingNotice(undefined, on, 'host')).toBeNull()
    expect(recordingNotice(on, on, 'host')).toBeNull()
    expect(recordingNotice(on, saved, 'host')).toBe('Enregistrement terminé : retrouvez-le dans Admin › Enregistrements.')
    expect(recordingNotice(on, full, 'host')).toBe("L'enregistrement s'est arrêté : le disque de l'ordinateur est plein.")
    expect(recordingNotice(on, ended, 'host')).toBeNull()
  })

  it("donne les avis de l'invité", () => {
    const guest = { guest: 'Atelier' }
    expect(recordingNotice(undefined, on, guest)).toBe('Atelier enregistre la session.')
    expect(recordingNotice(on, on, guest)).toBeNull()
    expect(recordingNotice(undefined, idle, guest)).toBeNull()
    setLangChoice('en')
    expect(recordingNotice(on, saved, guest)).toBe('Recording stopped.')
    setLangChoice('fr')
  })

  it('écrit les infobulles sans raccourci', () => {
    expect(recordingTooltip(idle)).toBe("Enregistrer la session sur l'ordinateur")
    expect(recordingTooltip(on)).toBe("Arrêter l'enregistrement")
    expect(recordingTooltip(idle, '⌃⌥⌘R')).toBe("Enregistrer la session sur l'ordinateur (⌃⌥⌘R)")
    expect(recordingTooltip({ ...idle, allowed: false })).toBe("L'enregistrement est coupé sur cet ordinateur (Admin › Enregistrements).")
  })

  it('écrit la taille et la durée des fichiers', () => {
    expect(fileSize(1_400_000_000)).toBe('1,4 Go')
    expect(fileSize(820_000_000)).toBe('820 Mo')
    expect(fileDuration(12 * 60_000)).toBe('12 min')
    expect(fileDuration(65 * 60_000)).toBe('1 h 05')
  })
})
