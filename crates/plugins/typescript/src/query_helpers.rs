//! Framework helpers consume the validated pagination plan without bypassing operations.
use poolster_core::{
    Api, Operation,
    pagination::{PaginationKind, SelectorSegment},
};
use serde_json::{Value, json};
use std::fmt::Write;

pub(crate) fn name(operation: &Operation) -> &str {
    operation
        .annotations
        .get("poolster.query.name")
        .and_then(Value::as_str)
        .unwrap_or(&operation.id)
}
pub(crate) fn is_read(operation: &Operation) -> bool {
    match operation
        .annotations
        .get("poolster.query.kind")
        .and_then(Value::as_str)
    {
        Some("query") => true,
        Some("mutation") => false,
        _ => operation.method == poolster_core::HttpMethod::Get,
    }
}
pub(crate) fn shared_runtime(exports: bool) -> String {
    let source = format!(
        "{}\n{}",
        include_str!("query_shared.ts.txt"),
        include_str!("query_pagination.ts.txt")
    );
    if exports {
        source.replace("\nconst __kaji", "\nexport const __kaji")
    } else {
        source
    }
}

pub(crate) fn exports(
    operation: &Operation,
    name: &str,
    pascal: &str,
    swr: bool,
) -> (Vec<String>, Vec<String>) {
    let mut values = Vec::new();
    let mut types = Vec::new();
    if is_read(operation) {
        if !swr {
            values.extend([format!("{name}Prefetch"), format!("{name}SuspenseOptions")]);
        }
        if operation.annotations.contains_key("x-kaji-pagination")
            || operation.annotations.contains_key("x-speakeasy-pagination")
        {
            values.extend([
                format!("{name}InfiniteKey"),
                format!("{name}InfiniteOptions"),
                format!("use{pascal}Infinite"),
            ]);
            if !swr {
                values.push(format!("{name}InfinitePrefetch"));
                types.push(format!("{pascal}InfiniteOverrides"));
            }
        }
    }
    (values, types)
}
fn plan(api: &Api, operation: &Operation) -> Option<Value> {
    let plan = poolster_core::pagination::normalize_pagination(api, operation, None).ok()??;
    let selector = |selector: Option<&poolster_core::pagination::Selector>| {
        selector.map(|s| {
            s.segments
                .iter()
                .map(|segment| match segment {
                    SelectorSegment::Field(field) => json!(field),
                    SelectorSegment::Index(index) => json!(index),
                })
                .collect::<Vec<_>>()
        })
    };
    let input = plan.inputs.iter().find(|i| i.role != "limit");
    let limit = plan.inputs.iter().find(|i| i.role == "limit");
    Some(
        json!({"kind": match plan.kind {PaginationKind::Cursor=>"cursor",PaginationKind::Page=>"page",PaginationKind::OffsetLimit=>"offset",PaginationKind::Url=>"url"}, "input": input.map(|i| json!({"name":i.name,"location":if i.location=="requestBody" {"body"} else {&i.location}})), "limit":limit.map(|i| json!({"name":i.name,"location":if i.location=="requestBody" {"body"} else {&i.location}})), "results":selector(plan.results.as_ref()),"continuation":selector(plan.continuation.as_ref())}),
    )
}
pub(crate) fn swr_mutation(function: &str, name: &str, pascal: &str, identity: &str) -> String {
    format!(
        "export const {name}MutationKey = (scope: PoolsterQueryScope = 'default') => [{identity:?}, scope] as const;\nexport const {name}MutationOptions = (scope: PoolsterQueryScope = 'default') => ({{ key: {name}MutationKey(scope), fetcher: (_key: ReturnType<typeof {name}MutationKey>, {{ arg }}: {{ arg: Parameters<typeof {function}>[0] }}) => {function}(arg) }});\nexport function use{pascal}(config: SWRMutationConfiguration<Awaited<ReturnType<typeof {function}>>, Error, ReturnType<typeof {name}MutationKey>, Parameters<typeof {function}>[0]> = {{}}, scope: PoolsterQueryScope = 'default') {{ const options = {name}MutationOptions(scope); return useSWRMutation(options.key, options.fetcher, config); }}\n"
    )
}
pub(crate) fn render(
    operation: &Operation,
    api: &Api,
    function: &str,
    name: &str,
    pascal: &str,
    framework: &str,
    swr: bool,
) -> String {
    if !is_read(operation) {
        return String::new();
    }
    let mut out = String::new();
    if !swr {
        let suspense_type = if framework == "@tanstack/react-query" {
            format!(
                "Omit<UseSuspenseQueryOptions<Awaited<ReturnType<typeof {function}>>, Error, TData, ReturnType<typeof {name}QueryKey>>, 'queryKey' | 'queryFn'>"
            )
        } else {
            format!("{pascal}QueryOverrides<TData>")
        };
        let _ = writeln!(
            out,
            "export function {name}SuspenseOptions<TData = Awaited<ReturnType<typeof {function}>>>(options: Parameters<typeof {function}>[0], query: {suspense_type} = {{}}, scope: PoolsterQueryScope = 'default') {{ return {name}QueryOptions(options, query, scope); }}\nexport function {name}Prefetch(queryClient: QueryClient, options: Parameters<typeof {function}>[0], scope: PoolsterQueryScope = 'default') {{ return queryClient.prefetchQuery({{ queryKey: {name}QueryKey(options, scope), queryFn: (context) => __kajiQueryCall(options, context.signal, {function}) }}); }}"
        );
        if framework == "@tanstack/react-query" {
            let _ = writeln!(
                out,
                "export function use{pascal}Suspense<TData = Awaited<ReturnType<typeof {function}>>>(options: Parameters<typeof {function}>[0], query: {suspense_type} = {{}}, scope: PoolsterQueryScope = 'default') {{ return useSuspenseQuery({name}SuspenseOptions(options, query, scope)); }}"
            );
        }
    }
    let Some(plan) = plan(api, operation) else {
        return out;
    };
    let _ = writeln!(
        out,
        "const __{name}Pagination = {plan} as const;\nexport const {name}InfiniteKey = (options: Parameters<typeof {function}>[0], scope: PoolsterQueryScope = 'default') => [...{name}QueryKey(options, scope), 'infinite'] as const;"
    );
    if swr {
        let _ = writeln!(
            out,
            "export function {name}InfiniteOptions(options: Parameters<typeof {function}>[0], scope: PoolsterQueryScope = 'default', pagination: PoolsterPaginationOptions = {{}}) {{\n const params: PoolsterPageParam[] = [__kajiInitial(options, __{name}Pagination)];\n return {{ getKey: (index: number, previous: Awaited<ReturnType<typeof {function}>> | null) => {{\n  if (index >= __kajiMaxPages(pagination)) throw new RangeError('pagination page limit exceeded');\n  if (index > 0) {{ if (previous === null) return null; const next = __kajiNext(previous, params[index - 1], options, __{name}Pagination); if (next === undefined || params.slice(0, index).includes(next)) return null; params[index] = next; }}\n  return [...{name}InfiniteKey(options, scope), params[index]] as const;\n }}, fetcher: (key: readonly unknown[]) => {function}(__kajiPageOptions(options, key[key.length - 1] as PoolsterPageParam, __{name}Pagination, pagination) as Parameters<typeof {function}>[0]) }};\n}}\nexport function use{pascal}Infinite(options: Parameters<typeof {function}>[0], config: SWRInfiniteConfiguration<Awaited<ReturnType<typeof {function}>>, Error> = {{}}, scope: PoolsterQueryScope = 'default', pagination: PoolsterPaginationOptions = {{}}) {{ const generated = {name}InfiniteOptions(options, scope, pagination); return useSWRInfinite(generated.getKey, generated.fetcher, {{ ...config, parallel: false }}); }}"
        );
        return out;
    }
    let data = format!("InfiniteData<Awaited<ReturnType<typeof {function}>>, PoolsterPageParam>");
    let observer = if framework == "@tanstack/vue-query" {
        format!(
            "Extract<UseInfiniteQueryOptions<Awaited<ReturnType<typeof {function}>>, Error, TData, ReturnType<typeof {name}InfiniteKey>, PoolsterPageParam>, {{ queryKey?: unknown }}>"
        )
    } else {
        format!(
            "InfiniteQueryObserverOptions<Awaited<ReturnType<typeof {function}>>, Error, TData, ReturnType<typeof {name}InfiniteKey>, PoolsterPageParam>"
        )
    };
    let _ = writeln!(
        out,
        "export type {pascal}InfiniteOverrides<TData = {data}> = Omit<{observer}, 'queryKey' | 'queryFn' | 'initialPageParam' | 'getNextPageParam'>;\nexport function {name}InfiniteOptions<TData = {data}>(options: Parameters<typeof {function}>[0], query: {pascal}InfiniteOverrides<TData> = {{}}, scope: PoolsterQueryScope = 'default', pagination: PoolsterPaginationOptions = {{}}): {pascal}InfiniteOverrides<TData> & {{ queryKey: ReturnType<typeof {name}InfiniteKey>; initialPageParam: PoolsterPageParam; queryFn: (context: QueryFunctionContext<ReturnType<typeof {name}InfiniteKey>, PoolsterPageParam>) => Promise<Awaited<ReturnType<typeof {function}>>>; getNextPageParam: (last: Awaited<ReturnType<typeof {function}>>, pages: Awaited<ReturnType<typeof {function}>>[], lastParam: PoolsterPageParam, params: PoolsterPageParam[]) => PoolsterPageParam | undefined }} {{\n return {{ ...query, queryKey: {name}InfiniteKey(options, scope), initialPageParam: __kajiInitial(options, __{name}Pagination), queryFn: (context) => __kajiQueryCall(__kajiPageOptions(options, context.pageParam, __{name}Pagination, pagination) as Parameters<typeof {function}>[0], context.signal, {function}), getNextPageParam: (last, pages, lastParam, params) => {{ const next = __kajiNext(last, lastParam, options, __{name}Pagination); if (next === undefined || params.includes(next)) return undefined; if (pages.length >= __kajiMaxPages(pagination)) throw new RangeError('pagination page limit exceeded'); return next; }} }};\n}}"
    );
    let returns = if framework == "@tanstack/vue-query" {
        ": UseInfiniteQueryReturnType<TData, Error>"
    } else {
        ""
    };
    let _ = writeln!(
        out,
        "export function use{pascal}Infinite<TData = {data}>(options: Parameters<typeof {function}>[0], query: {pascal}InfiniteOverrides<TData> = {{}}, scope: PoolsterQueryScope = 'default', pagination: PoolsterPaginationOptions = {{}}){returns} {{ return useInfiniteQuery({name}InfiniteOptions(options, query, scope, pagination)); }}\nexport function {name}InfinitePrefetch(queryClient: QueryClient, options: Parameters<typeof {function}>[0], scope: PoolsterQueryScope = 'default', pagination: PoolsterPaginationOptions = {{}}) {{ return queryClient.prefetchInfiniteQuery({name}InfiniteOptions(options, {{}}, scope, pagination)); }}"
    );
    if framework == "@tanstack/react-query" {
        let _ = writeln!(
            out,
            "export function use{pascal}SuspenseInfinite(options: Parameters<typeof {function}>[0], scope: PoolsterQueryScope = 'default', pagination: PoolsterPaginationOptions = {{}}) {{ return useSuspenseInfiniteQuery({name}InfiniteOptions(options, {{}}, scope, pagination)); }}"
        );
    }
    out
}
