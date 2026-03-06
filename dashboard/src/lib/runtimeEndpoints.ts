import type { ApiResponseWire } from '../types/wire'

export const DEFAULT_API_BASE = 'http://localhost:3000'
export const DEFAULT_WS_URL = 'ws://localhost:3000/ws'
export const LOCAL_API_FALLBACKS = ['http://localhost:8088', 'http://127.0.0.1:8088']
export const LOCAL_WS_FALLBACKS = ['ws://localhost:8088/ws', 'ws://127.0.0.1:8088/ws']

let preferredApiBase: string | null = null

export function normalizeBaseUrl(url: string): string {
  return url.endsWith('/') ? url.slice(0, -1) : url
}

export function normalizeWsUrl(url: string): string {
  return url.endsWith('/') ? url.slice(0, -1) : url
}

export function deriveWsUrlFromHttpBase(apiBase: string): string | null {
  try {
    const url = new URL(apiBase)
    url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'
    url.pathname = '/ws'
    url.search = ''
    url.hash = ''
    return normalizeWsUrl(url.toString())
  } catch {
    return null
  }
}

export function uniqueUrls(urls: Array<string | null | undefined>): string[] {
  return Array.from(
    new Set(
      urls
        .filter((url): url is string => Boolean(url))
        .map((url) => url.trim())
        .filter((url) => url.length > 0),
    ),
  )
}

export function buildApiCandidates(primaryApiBase?: string): string[] {
  return uniqueUrls([
    preferredApiBase,
    primaryApiBase ? normalizeBaseUrl(primaryApiBase) : null,
    ...LOCAL_API_FALLBACKS,
    DEFAULT_API_BASE,
  ]).map(normalizeBaseUrl)
}

export function buildWsCandidates(primaryWsUrl?: string, apiBase?: string): string[] {
  return uniqueUrls([
    primaryWsUrl ? normalizeWsUrl(primaryWsUrl) : null,
    apiBase ? deriveWsUrlFromHttpBase(apiBase) : null,
    ...LOCAL_WS_FALLBACKS,
    DEFAULT_WS_URL,
  ]).map(normalizeWsUrl)
}

export async function fetchApiFromCandidates<T>(
  path: string,
  candidates: string[],
  init?: RequestInit,
): Promise<{ data: T; baseUrl: string }> {
  let lastNetworkError: Error | null = null

  for (const candidate of candidates) {
    const baseUrl = normalizeBaseUrl(candidate)
    const url = `${baseUrl}${path}`

    try {
      const response = await fetch(url, {
        ...init,
        headers: {
          'ngrok-skip-browser-warning': 'true',
          ...(init?.headers ?? {}),
        },
      })

      if (!response.ok) {
        let apiError: string | null = null
        try {
          const payload = (await response.clone().json()) as ApiResponseWire<T>
          apiError = payload.error ?? null
        } catch {
          // Ignore non-JSON error bodies and fall back to generic HTTP detail.
        }

        throw new Error(apiError ?? `HTTP ${response.status} while requesting ${url}`)
      }

      const payload = (await response.json()) as ApiResponseWire<T>
      if (!payload.success || payload.data === undefined) {
        throw new Error(payload.error ?? `Invalid API response from ${url}`)
      }

      preferredApiBase = baseUrl
      return { data: payload.data, baseUrl }
    } catch (error) {
      if (error instanceof TypeError) {
        lastNetworkError = error
        continue
      }
      throw error
    }
  }

  throw lastNetworkError ?? new Error(`All API candidates failed for ${path}`)
}
