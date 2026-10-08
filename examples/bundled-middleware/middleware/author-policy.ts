import type { ClientMiddleware } from '../.poolster/client'

/** Author policy shipped and enabled by default; customers need no setup. */
export const authorPolicy: ClientMiddleware = async (request, next) => {
  const headers = new Headers(request.headers as HeadersInit)
  headers.set('X-SDK-Policy', 'bundled')
  return next({ ...request, headers })
}
