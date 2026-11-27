import { describe, expect, it } from 'vitest'
import { ApiError } from './api'
import { NetworkError } from './http'
import { app, errorText, notify } from './store.svelte'

describe('toasts d’erreur (WEB-07)', () => {
  it('une erreur du navigateur est dite dans la langue de la console', () => {
    const unreachable = new NetworkError().message
    expect(errorText(new TypeError('Failed to fetch'))).toBe(unreachable)
    expect(errorText(new DOMException('x', 'TimeoutError'))).toBe(unreachable)
    expect(errorText(new ApiError(409, 'Refusé par Admin'))).toBe('Refusé par Admin')
  })

  it('le même avis n’est pas reposé', () => {
    notify('error', 'a')
    const first = app.toast
    notify('error', 'a')
    expect(app.toast).toBe(first)
    notify('error', 'b')
    expect(app.toast?.text).toBe('b')
  })
})
