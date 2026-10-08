export interface SecurityCredentials {
    [name: string]: string | undefined;
}
export type SecurityDescriptor = {
    id: string;
    type: 'apiKey' | 'http' | 'oauth2';
    name?: string;
    in?: 'header' | 'query' | 'cookie';
    scheme?: string;
    scopes?: string[];
};
export declare class ApiError extends Error {
    readonly status: number;
    readonly body: unknown;
    constructor(status: number, body: unknown);
}
import { type AxiosInstance } from 'axios';
export interface RetryConfig {
    maxAttempts?: number;
    initialDelayMs?: number;
    maxDelayMs?: number;
}
export interface RequestHookContext {
    method: string;
    url: string;
    body?: unknown;
    path?: Record<string, unknown>;
    query: Record<string, unknown>;
    headers: Record<string, string>;
}
export interface ResponseHookContext {
    request: RequestHookContext;
    status: number;
    headers: Record<string, unknown>;
    data: unknown;
}
export interface ClientHooks {
    beforeRequest?: (request: RequestHookContext) => void | Promise<void>;
    afterResponse?: (response: ResponseHookContext) => void | Promise<void>;
    onError?: (error: unknown, request: RequestHookContext) => void | Promise<void>;
}
export interface ClientConfig {
    baseUrl?: string;
    apiKey?: string;
    apiKeyHeader?: string;
    apiKeyPrefix?: string;
    auth?: SecurityCredentials;
    headers?: Record<string, string>;
    client?: AxiosInstance;
    retry?: RetryConfig | false;
    hooks?: ClientHooks;
}
export type RequestConfig = {
    method: string;
    url: string;
    body?: unknown;
    path?: Record<string, unknown>;
    query?: Record<string, unknown>;
    headers?: Record<string, string>;
    throwOnError?: boolean;
    security?: SecurityDescriptor[][];
    contentType?: {
        request?: string;
    };
    responseType?: 'stream';
    styles?: unknown;
    paginationUrl?: string;
};
export type ClientInstance = (request: RequestConfig) => Promise<unknown>;
export type Options<T, ThrowOnError extends boolean> = T & {
    client?: ClientInstance;
    throwOnError?: ThrowOnError;
};
export type SuccessOf<T> = T[Extract<keyof T, `2${string}`>];
export type RequestResult<T, ThrowOnError extends boolean> = ThrowOnError extends true ? SuccessOf<T> : T[keyof T];
export type Unwrappable<T> = Promise<T> & {
    unwrap(): Promise<T>;
};
export type EventStreamResult<T> = AsyncIterable<T>;
export declare const createClient: (config?: ClientConfig) => ClientInstance;
export declare const toEventStream: <T>(response: Promise<unknown>) => Promise<EventStreamResult<T>>;
export declare const client: ClientInstance;
export declare const withUnwrap: <T>(promise: Promise<T>) => Unwrappable<T>;
