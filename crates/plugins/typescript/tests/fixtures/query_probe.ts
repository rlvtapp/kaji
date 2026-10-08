import { QueryClient, MutationObserver } from '@tanstack/react-query';
import { createClient } from './.poolster/client.js';
import { listContactsQueryKey, listContactsQueryOptions, createContactMutationOptions } from './ui/react.js';
import { listContactsQueryOptions as vueOptions, useListContacts as useVueContacts } from './ui/vue.js';
import { useListContacts as useSwrContacts } from './ui/swr.js';

const check = (condition: unknown, message: string) => { if (!condition) throw new Error(message); };
let calls = 0;
const transport = createClient({ baseUrl: 'https://unit.invalid', apiKey: 'must-not-appear-in-cache', fetch: async () => { calls++; return new Response('{}', { status: 200 }); } });
const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
const params = { client: transport };
const cached = listContactsQueryOptions(params, { staleTime: Infinity }, 'tenant-a');
await queryClient.fetchQuery(cached);
await queryClient.fetchQuery(listContactsQueryOptions({ client: createClient({ baseUrl: 'https://unit.invalid' }) }, { staleTime: Infinity }, 'tenant-a'));
check(calls === 1, 'equivalent inputs should reuse cache without client identity');
await queryClient.fetchQuery(listContactsQueryOptions(params, { staleTime: Infinity }, 'tenant-b'));
check(calls === 2, 'explicit tenant scope must isolate cache');
check(!JSON.stringify(listContactsQueryKey(params, 'tenant-a')).includes('must-not-appear'), 'cache key contains credentials');

await queryClient.invalidateQueries({ queryKey: cached.queryKey, exact: true });
await queryClient.fetchQuery(cached);
check(calls === 3, 'invalidated query must refetch through its generated operation');

const failingTransport = createClient({ baseUrl: 'https://unit.invalid', fetch: async () => new Response('failure', { status: 500 }) });
const queryFailure = await queryClient.fetchQuery(listContactsQueryOptions({ client: failingTransport, throwOnError: true }, { retry: false }, 'failure')).then(() => undefined, (error: unknown) => error);
check(queryFailure instanceof Error, 'SDK rejection must reach the framework query');
let mutationFailed = false;
let mutationSettled = false;
const failedObserver = new MutationObserver(queryClient, createContactMutationOptions({
  onError: (error) => { mutationFailed = error instanceof Error; },
  onSettled: () => { mutationSettled = true; },
}));
await failedObserver.mutate({ client: failingTransport, throwOnError: true }).then(() => { throw new Error('failed mutation unexpectedly succeeded'); }, () => undefined);
check(mutationFailed && mutationSettled, 'mutation error and settled callbacks must receive SDK failures');

let selected: string | undefined;
const selectedOptions = listContactsQueryOptions(params, { select: () => 'selected' });
selected = selectedOptions.select?.(await selectedOptions.queryFn({ signal: new AbortController().signal, queryKey: selectedOptions.queryKey, client: queryClient, meta: undefined }));
check(selected === 'selected', 'select override typing/value lost');
const vueSelected = vueOptions(params, { select: () => 'vue-selected', staleTime: 10 });
check(vueSelected.staleTime === 10, 'Vue override not forwarded');
void useVueContacts; void useSwrContacts;

let mutationContext = false;
const observer = new MutationObserver(queryClient, createContactMutationOptions<{ marker: string }>({
  onMutate: () => ({ marker: 'retained' }),
  onSuccess: (_data, _variables, context) => { mutationContext = context?.marker === 'retained'; },
}));
await observer.mutate(params);
check(mutationContext, 'mutation callbacks/context not forwarded');

let start!: () => void;
const started = new Promise<void>((resolve) => { start = resolve; });
let wireSignal: AbortSignal | null | undefined;
const slowTransport = createClient({ baseUrl: 'https://unit.invalid', fetch: async (_url, init) => new Promise<Response>((_resolve, reject) => {
  wireSignal = init?.signal; start();
  const fail = () => reject(new DOMException('cancelled', 'AbortError'));
  if (wireSignal?.aborted) fail(); else wireSignal?.addEventListener('abort', fail, { once: true });
}) });
const pendingOptions = listContactsQueryOptions({ client: slowTransport }, { retry: false }, 'cancel');
const pending = queryClient.fetchQuery(pendingOptions).catch(() => undefined);
await started;
await queryClient.cancelQueries({ queryKey: pendingOptions.queryKey });
await pending;
check(wireSignal?.aborted, 'TanStack cancellation did not reach HTTP driver');

const caller = new AbortController();
caller.abort('caller cancelled');
await listContactsQueryOptions({ client: transport, requestOptions: { signal: caller.signal } }).queryFn({ signal: new AbortController().signal, queryKey: cached.queryKey, client: queryClient, meta: undefined }).then(() => { throw new Error('caller cancellation ignored'); }, () => undefined);
queryClient.clear();
console.log('query factories, cache scopes, invalidation, errors, overrides, mutation context and cancellation passed');
