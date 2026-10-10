import {
  encodeGraphqlVariables,
  decodeGraphqlResult,
  type ScalarCodecMap,
  type ScalarShape,
  type ScalarInputShapes,
} from "./graphql-scalar-runtime.js";
/** Explicit path-based incremental dialect: deferSpec=20220824. */
export type DeepPartial<T> = T extends Date
  ? T
  : T extends readonly (infer U)[]
    ? DeepPartial<U>[]
    : T extends object
      ? { [K in keyof T]?: DeepPartial<T[K]> }
      : T;
export interface IncrementalError {
  message: string;
  path?: (string | number)[];
  locations?: { line: number; column: number }[];
  extensions?: Record<string, unknown>;
}
export interface IncrementalPatch {
  path: (string | number)[];
  label?: string;
  data?: Record<string, unknown> | null;
  items?: unknown[];
  errors?: IncrementalError[];
  extensions?: Record<string, unknown>;
}
export interface IncrementalSnapshot<T> {
  data?: DeepPartial<T> | null;
  errors: IncrementalError[];
  extensions?: Record<string, unknown>;
  patches: IncrementalPatch[];
  complete: boolean;
}
export interface IncrementalHttpOptions {
  endpoint: string;
  headers?: HeadersInit;
  fetch?: typeof fetch;
  signal?: AbortSignal;
  maxPartBytes?: number;
  scalarCodecs?: ScalarCodecMap;
}
const own = (o: object, key: PropertyKey) => Object.prototype.hasOwnProperty.call(o, key);
const object = (v: unknown): v is Record<string, unknown> =>
  v !== null && typeof v === "object" && !Array.isArray(v);
function errors(v: unknown): IncrementalError[] {
  if (v === undefined) return [];
  if (
    !Array.isArray(v) ||
    v.length === 0 ||
    v.some((e) => !object(e) || typeof e.message !== "string")
  )
    throw new Error("Invalid incremental GraphQL errors");
  return v as IncrementalError[];
}
function safePath(v: unknown): (string | number)[] {
  if (
    !Array.isArray(v) ||
    v.some((k) =>
      typeof k === "string"
        ? ["__proto__", "constructor", "prototype"].includes(k)
        : !Number.isSafeInteger(k) || k < 0,
    )
  )
    throw new Error("Invalid incremental patch path");
  return v;
}
function at(root: unknown, path: (string | number)[]): unknown {
  let value = root;
  for (const key of path) {
    if (Array.isArray(value)) {
      if (typeof key !== "number" || key >= value.length)
        throw new Error("Incremental path does not exist");
      value = value[key];
    } else if (object(value) && typeof key === "string" && own(value, key)) value = value[key];
    else throw new Error("Incremental path does not exist");
  }
  return value;
}
function merge(root: unknown, patch: IncrementalPatch): unknown {
  const path = patch.path;
  if (own(patch, "items")) {
    if (
      !Array.isArray(patch.items) ||
      path.length === 0 ||
      typeof path[path.length - 1] !== "number"
    )
      throw new Error("Invalid stream items path");
    const list = at(root, path.slice(0, -1));
    const index = path[path.length - 1] as number;
    if (!Array.isArray(list) || index !== list.length)
      throw new Error("Stream patches must append contiguous items");
    list.push(...patch.items);
    return root;
  }
  if (!own(patch, "data") || (patch.data !== null && !object(patch.data)))
    throw new Error("Invalid deferred data");
  const target = at(root, path);
  if (patch.data !== null) {
    if (!object(target)) throw new Error("Deferred target must be an object");
    for (const [key, value] of Object.entries(patch.data)) {
      safePath([key]);
      target[key] = value;
    }
    return root;
  }
  if (path.length === 0) return null;
  const parent = at(root, path.slice(0, -1));
  const key = path[path.length - 1];
  if (Array.isArray(parent) && typeof key === "number") parent[key] = null;
  else if (object(parent) && typeof key === "string") parent[key] = null;
  else throw new Error("Invalid deferred null target");
  return root;
}
export function applyIncrementalPayload<T>(
  previous: IncrementalSnapshot<T> | undefined,
  payload: unknown,
): IncrementalSnapshot<T> {
  if (!object(payload) || own(payload, "pending") || own(payload, "completed"))
    throw new Error("Unsupported incremental response dialect");
  if (previous?.complete) throw new Error("Payload after incremental completion");
  if (typeof payload.hasNext !== "boolean") throw new Error("Incremental payload requires hasNext");
  const next: IncrementalSnapshot<T> = previous
    ? structuredClone(previous)
    : { errors: [], patches: [], complete: false };
  next.patches = [];
  if (!previous) {
    if ((!own(payload, "data") && !own(payload, "errors")) || own(payload, "incremental"))
      throw new Error("Invalid initial incremental envelope");
    if (own(payload, "data")) {
      if (payload.data !== null && !object(payload.data))
        throw new Error("Invalid initial GraphQL data");
      next.data = structuredClone(payload.data) as DeepPartial<T> | null;
    }
  } else if (own(payload, "data")) throw new Error("Repeated initial data");
  next.errors.push(...errors(payload.errors));
  if (payload.extensions !== undefined) {
    if (!object(payload.extensions)) throw new Error("Invalid extensions");
    next.extensions = structuredClone(payload.extensions);
  }
  if (own(payload, "incremental")) {
    if (!Array.isArray(payload.incremental) || payload.incremental.length === 0)
      throw new Error("Invalid incremental patches");
    for (const item of payload.incremental) {
      if (!object(item) || own(item, "id") || own(item, "subPath"))
        throw new Error("Unsupported incremental patch dialect");
      const path = safePath(item.path);
      if (item.label !== undefined && typeof item.label !== "string")
        throw new Error("Invalid patch label");
      if (own(item, "items") === own(item, "data"))
        throw new Error("Patch needs exactly one of data/items");
      const patch = { ...item, path } as unknown as IncrementalPatch;
      next.errors.push(...errors(item.errors));
      next.data = merge(next.data, patch) as DeepPartial<T> | null;
      next.patches.push(structuredClone(patch));
    }
  }
  next.complete = payload.hasNext === false;
  return next;
}
async function* multipart(
  response: Response,
  boundary: string,
  limit: number,
): AsyncGenerator<unknown> {
  if (!response.body) throw new Error("Incremental response body is missing");
  const reader = response.body.getReader();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const marker = "--" + boundary;
  let buffer = "";
  let started = false;
  let closed = false;
  try {
    while (!closed) {
      const chunk = await reader.read();
      buffer += chunk.done ? decoder.decode() : decoder.decode(chunk.value, { stream: true });
      if (buffer.length > limit) throw new Error("Incremental multipart part exceeds limit");
      if (!started) {
        const first = buffer.indexOf(marker + "\r\n");
        if (first < 0) {
          if (chunk.done) throw new Error("Missing multipart boundary");
          continue;
        }
        if (buffer.slice(0, first).trim()) throw new Error("Unexpected multipart preamble");
        buffer = buffer.slice(first + marker.length + 2);
        started = true;
      }
      while (true) {
        const end = buffer.indexOf("\r\n" + marker);
        if (end < 0 || buffer.length < end + 2 + marker.length + 2) break;
        const part = buffer.slice(0, end);
        const suffix = buffer.slice(end + 2 + marker.length, end + 2 + marker.length + 2);
        if (suffix !== "--" && suffix !== "\r\n")
          throw new Error("Invalid multipart boundary suffix");
        const headersEnd = part.indexOf("\r\n\r\n");
        if (
          headersEnd < 0 ||
          !/^content-type:\s*application\/json\s*$/im.test(part.slice(0, headersEnd))
        )
          throw new Error("Multipart part requires application/json");
        yield JSON.parse(part.slice(headersEnd + 4));
        buffer = buffer.slice(end + 2 + marker.length + 2);
        if (suffix === "--") {
          closed = true;
          if (buffer.trim()) throw new Error("Unexpected multipart epilogue");
          break;
        }
      }
      if (chunk.done && !closed) throw new Error("Truncated incremental multipart response");
    }
  } finally {
    await reader.cancel();
    reader.releaseLock();
  }
}
export async function* executeIncremental<T>(
  options: IncrementalHttpOptions,
  operationName: string,
  query: string,
  variables: object,
  variableShape: ScalarShape = null,
  resultShape: ScalarShape = null,
  inputs: ScalarInputShapes = {},
): AsyncGenerator<IncrementalSnapshot<T>> {
  const headers = new Headers(options.headers);
  headers.set("Content-Type", "application/json");
  headers.set(
    "Accept",
    "multipart/mixed;deferSpec=20220824, application/graphql-response+json, application/json",
  );
  const response = await (options.fetch ?? fetch)(options.endpoint, {
    method: "POST",
    headers,
    body: JSON.stringify({
      query,
      operationName,
      variables: encodeGraphqlVariables(variables, variableShape, inputs, options.scalarCodecs),
    }),
    signal: options.signal,
  });
  if (!response.ok) throw new Error(`GraphQL HTTP status ${response.status}`);
  const type = response.headers.get("Content-Type") ?? "";
  if (!/^multipart\/mixed(?:;|$)/i.test(type)) {
    if (!/^application\/(?:graphql-response\+)?json(?:;|$)/i.test(type))
      throw new Error("Unsupported GraphQL response Content-Type");
    const payload: unknown = await response.json();
    if (!object(payload) || own(payload, "hasNext") || own(payload, "incremental"))
      throw new Error("Incremental payload requires multipart transport");
    yield decodeSnapshot(
      applyIncrementalPayload<T>(undefined, { ...payload, hasNext: false }),
      resultShape,
      inputs,
      options.scalarCodecs,
    );
    return;
  }
  const version = /;\s*deferSpec\s*=\s*"?([^;"\s]+)/i.exec(type)?.[1];
  if ((version !== undefined && version !== "20220824") || /;\s*incrementalSpec\s*=/i.test(type))
    throw new Error("Unsupported incremental response version");
  const boundary = /;\s*boundary\s*=\s*(?:"([^"\r\n]+)"|([^;\s]+))/i.exec(type);
  const token = boundary?.[1] ?? boundary?.[2];
  if (!token || token.length > 70 || !/^[\x20-\x7e]+$/.test(token))
    throw new Error("Invalid multipart boundary");
  const limit = options.maxPartBytes ?? 8 * 1024 * 1024;
  if (!Number.isSafeInteger(limit) || limit < 1) throw new Error("Invalid multipart part limit");
  let previous: IncrementalSnapshot<T> | undefined;
  for await (const payload of multipart(response, token, limit)) {
    previous = applyIncrementalPayload<T>(previous, payload);
    yield decodeSnapshot(previous, resultShape, inputs, options.scalarCodecs);
  }
  if (!previous?.complete) throw new Error("Incremental response ended before hasNext:false");
}

function decodeSnapshot<T>(
  snapshot: IncrementalSnapshot<T>,
  shape: ScalarShape,
  inputs: ScalarInputShapes,
  codecs: ScalarCodecMap | undefined,
): IncrementalSnapshot<T> {
  if (snapshot.data === null || snapshot.data === undefined || !codecs) return snapshot;
  const result = decodeGraphqlResult(
    { kind: "success" as const, data: snapshot.data },
    shape,
    inputs,
    codecs,
  );
  return { ...snapshot, data: result.data };
}
