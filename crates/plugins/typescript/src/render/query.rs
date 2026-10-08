//! Query factories, hooks and typed framework overrides.
use super::*;

/// Emits TanStack React Query keys and query/mutation hooks over generated operation functions.
#[derive(Default)]
pub struct TypeScriptReactQuery;

impl TypeScriptReactQuery {
    pub fn generate(&self, api: &Api, options: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
        Ok(vec![GeneratedFile::new(
            extra_output_path(options, "typescript", "react-query.ts"),
            render_hooks(api, options, "@tanstack/react-query", false),
        )?])
    }
}

/// Emits TanStack Vue Query composables over generated operation functions.
#[derive(Default)]
pub struct TypeScriptVueQuery;

impl TypeScriptVueQuery {
    pub fn generate(&self, api: &Api, options: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
        Ok(vec![GeneratedFile::new(
            extra_output_path(options, "typescript", "vue-query.ts"),
            render_hooks(api, options, "@tanstack/vue-query", false),
        )?])
    }
}

/// Emits SWR hooks backed by generated operation functions.
#[derive(Default)]
pub struct TypeScriptSwr;

impl TypeScriptSwr {
    pub fn generate(&self, api: &Api, options: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
        Ok(vec![GeneratedFile::new(
            extra_output_path(options, "typescript", "swr.ts"),
            render_hooks(api, options, "swr", true),
        )?])
    }
}

pub(crate) fn query_exports(
    api: &Api,
    options: &ArtifactOptions,
    swr: bool,
) -> (Vec<String>, Vec<String>) {
    let mut values = Vec::new();
    let mut types = Vec::new();
    for operation in &api.operations {
        let name = operation_identifier(crate::query_helpers::name(operation), options);
        let pascal = type_identifier(crate::query_helpers::name(operation));
        if crate::query_helpers::is_read(operation) {
            values.extend([format!("{name}QueryKey"), format!("use{pascal}")]);
            if !swr {
                values.push(format!("{name}QueryOptions"));
                types.push(format!("{pascal}QueryOverrides"));
            }
        } else {
            values.extend([
                format!("{name}MutationKey"),
                format!("{name}MutationOptions"),
                format!("use{pascal}"),
            ]);
            if !swr {
                types.push(format!("{pascal}MutationOverrides"));
            }
        }
        let (extra_values, extra_types) =
            crate::query_helpers::exports(operation, &name, &pascal, swr);
        values.extend(extra_values);
        types.extend(extra_types);
    }
    (values, types)
}

fn render_hooks(api: &Api, options: &ArtifactOptions, framework: &str, swr: bool) -> String {
    let mut prepared = crate::symbols::prepare(api);
    for (original, operation) in api.operations.iter().zip(&mut prepared.operations) {
        operation
            .annotations
            .entry("kaji.query.operation_id".into())
            .or_insert_with(|| json!(original.id));
    }
    let api = &prepared;
    let mut output = if swr {
        format!(
            "{NOTICE}\nimport useSWR, {{ type SWRConfiguration }} from 'swr';\nimport useSWRInfinite, {{ type SWRInfiniteConfiguration }} from 'swr/infinite';\nimport useSWRMutation from 'swr/mutation';\nimport type {{ SWRMutationConfiguration }} from 'swr/mutation';\n"
        )
    } else {
        format!(
            "{NOTICE}\nimport {{ useMutation, useQuery, type QueryObserverOptions, type MutationObserverOptions, type QueryFunctionContext, useInfiniteQuery, type InfiniteQueryObserverOptions, type InfiniteData, type QueryClient }} from '{framework}';\n"
        )
    };
    if !swr && framework == "@tanstack/react-query" {
        output.push_str("import { useSuspenseQuery, useSuspenseInfiniteQuery, type UseSuspenseQueryOptions } from '@tanstack/react-query';\n");
    }
    if !swr && framework == "@tanstack/vue-query" {
        output.push_str(
            "import type { UseQueryOptions, UseMutationReturnType, UseInfiniteQueryOptions, UseInfiniteQueryReturnType } from '@tanstack/vue-query';\n",
        );
    }
    output.push_str("// __kaji_shared_start\n");
    output.push_str(&crate::query_helpers::shared_runtime(false));
    output.push_str("// __kaji_shared_end\n");
    for operation in &api.operations {
        output.push_str(&render_query_operation(
            api, operation, options, framework, swr,
        ));
    }
    output
}

pub(crate) fn render_query_operation(
    api: &Api,
    operation: &kaji_core::Operation,
    options: &ArtifactOptions,
    framework: &str,
    swr: bool,
) -> String {
    let mut output = String::new();
    let read = crate::query_helpers::is_read(operation);
    let function = crate::sdk::lower_camel_identifier(&operation.id);
    let name = operation_identifier(crate::query_helpers::name(operation), options);
    let pascal = type_identifier(crate::query_helpers::name(operation));
    let identity = operation
        .annotations
        .get("kaji.query.operation_id")
        .and_then(Value::as_str)
        .unwrap_or(&operation.id);
    let group = if options.group_by_tag {
        format!("{}/", crate::sdk::operation_group(operation))
    } else {
        String::new()
    };
    let import = format!(
        "{}/{group}{function}",
        options.clients_import.trim_end_matches('/')
    );
    let _ = writeln!(
        output,
        "import {{ {function} }} from {};",
        js_string(&import)
    );
    if read {
        let _ = writeln!(
            output,
            "export const {name}QueryKey = (options: Parameters<typeof {function}>[0], scope: KajiQueryScope = 'default') => [{}, scope, __kajiInputs(options)] as const;",
            js_string(identity)
        );
        if swr {
            let _ = writeln!(
                output,
                "export function use{pascal}(options: Parameters<typeof {function}>[0], config: SWRConfiguration<Awaited<ReturnType<typeof {function}>>, Error> = {{}}, scope: KajiQueryScope = 'default') {{ return useSWR({name}QueryKey(options, scope), () => {function}(options), config); }}\n"
            );
        } else {
            let query_type = if framework == "@tanstack/vue-query" {
                format!(
                    "Extract<UseQueryOptions<Awaited<ReturnType<typeof {function}>>, Error, TData, Awaited<ReturnType<typeof {function}>>, ReturnType<typeof {name}QueryKey>>, {{ queryKey?: unknown }}>"
                )
            } else {
                format!(
                    "QueryObserverOptions<Awaited<ReturnType<typeof {function}>>, Error, TData, Awaited<ReturnType<typeof {function}>>, ReturnType<typeof {name}QueryKey>>"
                )
            };
            let _ = writeln!(
                output,
                "export type {pascal}QueryOverrides<TData = Awaited<ReturnType<typeof {function}>>> = Omit<{query_type}, 'queryKey' | 'queryFn'>;\nexport function {name}QueryOptions<TData = Awaited<ReturnType<typeof {function}>>>(options: Parameters<typeof {function}>[0], query: {pascal}QueryOverrides<TData> = {{}}, scope: KajiQueryScope = 'default'): {pascal}QueryOverrides<TData> & {{ queryKey: ReturnType<typeof {name}QueryKey>; queryFn: (context: QueryFunctionContext<ReturnType<typeof {name}QueryKey>>) => Promise<Awaited<ReturnType<typeof {function}>>> }} {{\n  return {{ ...query, queryKey: {name}QueryKey(options, scope), queryFn: (context: QueryFunctionContext<ReturnType<typeof {name}QueryKey>>) => __kajiQueryCall(options, context.signal, {function}) }};\n}}\nexport function use{pascal}<TData = Awaited<ReturnType<typeof {function}>>>(options: Parameters<typeof {function}>[0], query: {pascal}QueryOverrides<TData> = {{}}, scope: KajiQueryScope = 'default') {{ return useQuery({name}QueryOptions(options, query, scope)); }}\n"
            );
        }
    } else if swr {
        output.push_str(&crate::query_helpers::swr_mutation(
            &function, &name, &pascal, identity,
        ));
    } else {
        let hook_return = if framework == "@tanstack/vue-query" {
            format!(
                ": UseMutationReturnType<Awaited<ReturnType<typeof {function}>>, Error, Parameters<typeof {function}>[0], TContext>"
            )
        } else {
            String::new()
        };
        let _ = writeln!(
            output,
            "export const {name}MutationKey = () => [{}] as const;\nexport type {pascal}MutationOverrides<TContext = unknown> = Omit<MutationObserverOptions<Awaited<ReturnType<typeof {function}>>, Error, Parameters<typeof {function}>[0], TContext>, 'mutationFn'>;\nexport function {name}MutationOptions<TContext = unknown>(mutation: {pascal}MutationOverrides<TContext> = {{}}): {pascal}MutationOverrides<TContext> & {{ mutationKey: readonly unknown[]; mutationFn: (options: Parameters<typeof {function}>[0]) => ReturnType<typeof {function}> }} {{ return {{ ...mutation, mutationKey: mutation.mutationKey ?? {name}MutationKey(), mutationFn: (options: Parameters<typeof {function}>[0]) => {function}(options) }}; }}\nexport function use{pascal}<TContext = unknown>(mutation: {pascal}MutationOverrides<TContext> = {{}}){hook_return} {{ return useMutation({name}MutationOptions(mutation)); }}\n",
            js_string(identity)
        );
    }
    output.push_str(&crate::query_helpers::render(
        operation, api, &function, &name, &pascal, framework, swr,
    ));

    output
}
