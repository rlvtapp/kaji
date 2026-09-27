export class ApiError extends Error {
    status;
    body;
    constructor(status, body) {
        super(`Request failed: ${status}`);
        this.status = status;
        this.body = body;
    }
}
const resolveUrl = (template, path, query) => {
    const url = template.replace(/\{([^}]+)\}/g, (_match, key) => encodeURIComponent(String(path?.[key] ?? `{${key}}`)));
    const params = new URLSearchParams();
    for (const [key, value] of Object.entries(query ?? {})) {
        if (value === undefined || value === null)
            continue;
        for (const item of Array.isArray(value) ? value : [value])
            params.append(key, String(item));
    }
    const search = params.toString();
    return search ? `${url}${url.includes('?') ? '&' : '?'}${search}` : url;
};
const resolvePaginationUrl = (nextUrl, baseUrl) => {
    // A continuation URL comes from a response body. Only accept an absolute
    // URL on the configured API origin, or a root-relative URL when no base URL
    // is configured. This prevents credentials from being sent to another host.
    if (!baseUrl) {
        if (!nextUrl.startsWith('/') || nextUrl.startsWith('//'))
            throw new TypeError('Pagination URL must be root-relative when baseUrl is not configured');
        return nextUrl;
    }
    const base = new URL(baseUrl);
    const target = new URL(nextUrl, base);
    if (target.origin !== base.origin)
        throw new TypeError('Pagination URL must use the configured API origin');
    return target.toString();
};
const requestBody = (body, headers) => {
    if (body === undefined || body === null || typeof body === 'string' || body instanceof FormData || body instanceof URLSearchParams || body instanceof Blob)
        return body;
    if (!headers.has('content-type'))
        headers.set('content-type', 'application/json');
    return JSON.stringify(body);
};
const applySecurity = (headers, query, security, credentials) => {
    if (!security)
        return;
    const values = { ...credentials };
    const selected = security.find((alternative) => alternative.every((scheme) => values[scheme.id]));
    if (!selected)
        return;
    for (const scheme of selected) {
        const value = values[scheme.id];
        if (!value)
            continue;
        if (scheme.type === 'apiKey') {
            const name = scheme.name;
            if (!name)
                continue;
            if (scheme.in === 'query')
                query[name] = value;
            else
                headers.set(name, value);
        }
        else
            headers.set('authorization', `${scheme.scheme === 'basic' ? 'Basic' : 'Bearer'} ${value}`);
    }
};
const retryableStatus = (status) => status === 408 || status === 429 || status === 500 || status === 502 || status === 503 || status === 504;
const retryAllowed = (method, headers) => ['GET', 'PUT', 'PATCH', 'DELETE'].includes(method.toUpperCase()) || (method.toUpperCase() === 'POST' && headers.has('idempotency-key'));
const retryDelay = async (attempt, retry, retryAfter) => {
    const retryAfterMs = retryAfter && /^\d+(?:\.\d+)?$/.test(retryAfter) ? Number(retryAfter) * 1000 : undefined;
    const exponential = (retry.initialDelayMs ?? 250) * 2 ** attempt;
    const delay = Math.min(retryAfterMs ?? exponential, retry.maxDelayMs ?? 8_000);
    await new Promise((resolve) => setTimeout(resolve, delay));
};
export const createClient = (config = {}) => async ({ method, url, body, path, query, headers, throwOnError: _throwOnError, security, responseType, paginationUrl }) => {
    const mergedHeaders = new Headers(config.headers);
    if (config.apiKey)
        mergedHeaders.set(config.apiKeyHeader ?? 'authorization', `${config.apiKeyPrefix ?? 'Bearer '}${config.apiKey}`);
    new Headers(headers).forEach((value, key) => mergedHeaders.set(key, value));
    const resolvedQuery = { ...(query ?? {}) };
    applySecurity(mergedHeaders, resolvedQuery, security, config.auth);
    const requestUrl = paginationUrl ? resolvePaginationUrl(paginationUrl, config.baseUrl) : `${config.baseUrl ?? ''}${resolveUrl(url, path, resolvedQuery)}`;
    const request = { method, url: requestUrl, body, path, query: resolvedQuery, headers: mergedHeaders };
    await config.hooks?.beforeRequest?.(request);
    const retry = config.retry === false ? undefined : config.retry ?? {};
    const maxAttempts = retry && retryAllowed(method, mergedHeaders) ? Math.max(1, retry.maxAttempts ?? 3) : 1;
    let response;
    for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
        try {
            response = await (config.fetch ?? globalThis.fetch)(requestUrl, { method, body: requestBody(body, mergedHeaders), headers: mergedHeaders });
            if (attempt + 1 < maxAttempts && retryableStatus(response.status)) {
                await retryDelay(attempt, retry ?? {}, response.headers.get('retry-after'));
                continue;
            }
            break;
        }
        catch (error) {
            if (attempt + 1 >= maxAttempts) {
                await config.hooks?.onError?.(error, request);
                throw error;
            }
            await retryDelay(attempt, retry ?? {});
        }
    }
    if (!response.ok && _throwOnError !== false) {
        const error = new ApiError(response.status, await response.text());
        await config.hooks?.onError?.(error, request);
        throw error;
    }
    await config.hooks?.afterResponse?.({ request, status: response.status, response: response.clone() });
    if (responseType === 'stream')
        return response;
    if (response.status === 204)
        return undefined;
    const contentType = response.headers.get('content-type') ?? '';
    if (contentType.includes('application/json') || contentType.includes('+json'))
        return response.json();
    if (contentType.startsWith('text/'))
        return response.text();
    return response.arrayBuffer();
};
export const toEventStream = async (response) => {
    const raw = await response;
    if (!(raw instanceof Response) || !raw.body)
        throw new TypeError('SSE requires a Fetch Response with a readable body');
    const reader = raw.body.getReader();
    const decoder = new TextDecoder();
    return (async function* () {
        let buffer = '';
        while (true) {
            const next = await reader.read();
            if (next.done)
                break;
            buffer += decoder.decode(next.value, { stream: true });
            const events = buffer.split(/\r?\n\r?\n/);
            buffer = events.pop() ?? '';
            for (const event of events) {
                const data = event.split(/\r?\n/).filter((line) => line.startsWith('data:')).map((line) => line.slice(5).trimStart()).join('\n');
                if (!data)
                    continue;
                try {
                    yield JSON.parse(data);
                }
                catch {
                    yield data;
                }
            }
        }
    })();
};
export const client = createClient();
export const withUnwrap = (promise) => Object.assign(promise, { unwrap: () => promise });
