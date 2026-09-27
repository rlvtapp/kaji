export interface SecurityCredentials {
  [name: string]: string | undefined
}
export type SecurityDescriptor = { id: string; type: 'apiKey' | 'http' | 'oauth2'; name?: string; in?: 'header' | 'query' | 'cookie'; scheme?: string; scopes?: string[] }
export class ApiError extends Error {
  constructor(public readonly status: number, public readonly body: unknown) { super(`Request failed: ${status}`) }
}
import axios, { type AxiosInstance } from 'axios'
export interface RetryConfig { maxAttempts?: number; initialDelayMs?: number; maxDelayMs?: number }
export interface RequestHookContext { method: string; url: string; body?: unknown; path?: Record<string, unknown>; query: Record<string, unknown>; headers: Record<string, string> }
export interface ResponseHookContext { request: RequestHookContext; status: number; headers: Record<string, unknown>; data: unknown }
export interface ClientHooks { beforeRequest?: (request: RequestHookContext) => void | Promise<void>; afterResponse?: (response: ResponseHookContext) => void | Promise<void>; onError?: (error: unknown, request: RequestHookContext) => void | Promise<void> }
export interface ClientConfig { baseUrl?: string; apiKey?: string; apiKeyHeader?: string; apiKeyPrefix?: string; auth?: SecurityCredentials; headers?: Record<string, string>; client?: AxiosInstance; retry?: RetryConfig | false; hooks?: ClientHooks }
export type RequestConfig = { method: string; url: string; body?: unknown; path?: Record<string, unknown>; query?: Record<string, unknown>; headers?: Record<string, string>; throwOnError?: boolean; security?: SecurityDescriptor[][]; contentType?: { request?: string }; responseType?: 'stream'; styles?: unknown; paginationUrl?: string }
export type ClientInstance = (request: RequestConfig) => Promise<unknown>
export type Options<T, ThrowOnError extends boolean> = T & { client?: ClientInstance; throwOnError?: ThrowOnError }
export type SuccessOf<T> = T[Extract<keyof T, `2${string}`>]
export type RequestResult<T, ThrowOnError extends boolean> = ThrowOnError extends true ? SuccessOf<T> : T[keyof T]
export type Unwrappable<T> = Promise<T> & { unwrap(): Promise<T> }
export type EventStreamResult<T> = AsyncIterable<T>
const resolvePath = (template: string, path?: Record<string, unknown>) => template.replace(/\{([^}]+)\}/g, (_match, key) => encodeURIComponent(String(path?.[key] ?? `{${key}}`)))
const resolvePaginationUrl = (nextUrl: string, baseUrl?: string): string => {
  // See the Fetch runtime: a response-supplied continuation must never be
  // allowed to carry configured credentials to a different origin.
  if (!baseUrl) {
    if (!nextUrl.startsWith('/') || nextUrl.startsWith('//')) throw new TypeError('Pagination URL must be root-relative when baseUrl is not configured')
    return nextUrl
  }
  const base = new URL(baseUrl)
  const target = new URL(nextUrl, base)
  if (target.origin !== base.origin) throw new TypeError('Pagination URL must use the configured API origin')
  return target.toString()
}
const applySecurity = (headers: Record<string, string>, query: Record<string, unknown>, security: SecurityDescriptor[][] | undefined, credentials?: SecurityCredentials) => {
  if (!security) return
  const values: Record<string, string | undefined> = { ...credentials }
  const selected = security.find((alternative) => alternative.every((scheme) => values[scheme.id]))
  if (!selected) return
  for (const scheme of selected) {
    const value = values[scheme.id]
    if (!value) continue
    if (scheme.type === 'apiKey') {
      const name = scheme.name
      if (!name) continue
      if (scheme.in === 'query') query[name] = value
      else headers[name] = value
    } else headers.authorization = `${scheme.scheme === 'basic' ? 'Basic' : 'Bearer'} ${value}`
  }
}
const retryableStatus = (status: number) => status === 408 || status === 429 || status === 500 || status === 502 || status === 503 || status === 504
const retryAllowed = (method: string, headers: Record<string, string>) => ['GET', 'PUT', 'PATCH', 'DELETE'].includes(method.toUpperCase()) || (method.toUpperCase() === 'POST' && Object.keys(headers).some((key) => key.toLowerCase() === 'idempotency-key'))
const retryDelay = async (attempt: number, retry: RetryConfig, retryAfter?: string) => {
  const retryAfterMs = retryAfter && /^\d+(?:\.\d+)?$/.test(retryAfter) ? Number(retryAfter) * 1000 : undefined
  const exponential = (retry.initialDelayMs ?? 250) * 2 ** attempt
  const delay = Math.min(retryAfterMs ?? exponential, retry.maxDelayMs ?? 8_000)
  await new Promise<void>((resolve) => setTimeout(resolve, delay))
}
export const createClient = (config: ClientConfig = {}): ClientInstance => {
  const headers = { ...config.headers }
  if (config.apiKey) headers[config.apiKeyHeader ?? 'authorization'] = `${config.apiKeyPrefix ?? 'Bearer '}${config.apiKey}`
  const instance = config.client ?? axios.create({ baseURL: config.baseUrl, headers })
  return async ({ method, url, body, path, query, headers: requestHeaders, throwOnError: _throwOnError, security, responseType, paginationUrl }) => {
    const resolvedHeaders = { ...headers, ...requestHeaders }
    const resolvedQuery = { ...(query ?? {}) }
    applySecurity(resolvedHeaders, resolvedQuery, security, config.auth)
    const requestUrl = paginationUrl ? resolvePaginationUrl(paginationUrl, config.baseUrl) : resolvePath(url, path)
    const request = { method, url: requestUrl, body, path, query: resolvedQuery, headers: resolvedHeaders }
    await config.hooks?.beforeRequest?.(request)
    const retry = config.retry === false ? undefined : config.retry ?? {}
    const maxAttempts = retry && retryAllowed(method, resolvedHeaders) ? Math.max(1, retry.maxAttempts ?? 3) : 1
    for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
      try {
        const response = await instance.request({ method, url: requestUrl, data: body, params: resolvedQuery, headers: resolvedHeaders, responseType: responseType === 'stream' ? 'stream' : undefined, validateStatus: () => true })
        if (attempt + 1 < maxAttempts && retryableStatus(response.status)) {
          await retryDelay(attempt, retry ?? {}, response.headers['retry-after'])
          continue
        }
        if (response.status >= 400 && _throwOnError !== false) throw new ApiError(response.status, response.data)
        await config.hooks?.afterResponse?.({ request, status: response.status, headers: response.headers as Record<string, unknown>, data: response.data })
        return response.data
      } catch (error) {
        if (error instanceof ApiError || attempt + 1 >= maxAttempts) {
          await config.hooks?.onError?.(error, request)
          throw error
        }
        await retryDelay(attempt, retry ?? {})
      }
    }
    throw new Error('Kaji retry loop completed without a response')
  }
}
export const toEventStream = async <T>(response: Promise<unknown>): Promise<EventStreamResult<T>> => {
  const raw = await response
  const stream = raw instanceof Response ? raw.body : raw as ReadableStream<Uint8Array> | null
  if (!stream || typeof stream.getReader !== 'function') throw new TypeError('SSE requires an Axios stream-capable adapter')
  const reader = stream.getReader()
  const decoder = new TextDecoder()
  return (async function* (): AsyncGenerator<T> {
    let buffer = ''
    while (true) {
      const next = await reader.read()
      if (next.done) break
      buffer += decoder.decode(next.value, { stream: true })
      const events = buffer.split(/\r?\n\r?\n/)
      buffer = events.pop() ?? ''
      for (const event of events) {
        const data = event.split(/\r?\n/).filter((line) => line.startsWith('data:')).map((line) => line.slice(5).trimStart()).join('\n')
        if (!data) continue
        try { yield JSON.parse(data) as T } catch { yield data as T }
      }
    }
  })()
}
export const client = createClient()
export const withUnwrap = <T>(promise: Promise<T>): Unwrappable<T> => Object.assign(promise, { unwrap: () => promise })
