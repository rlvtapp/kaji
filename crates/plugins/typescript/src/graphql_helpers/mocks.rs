use super::*;
use poolster_core::native::GraphqlOperationKind;
use std::{fmt::Write, path::Path};
fn envelope(client: &GraphqlClient, target: &Path) -> Result<String> {
    let error = crate::Symbol {
        module: client.runtime_module.clone(),
        name: "GraphqlError".into(),
    };
    Ok(format!(
        "{}import type {{ GraphqlError }} from {};\nexport type GraphqlMockResponse<T> = {{data:T;errors?:[GraphqlError,...GraphqlError[]]}} | {{data?:null;errors:[GraphqlError,...GraphqlError[]]}};\n",
        imports(client, target)?,
        serde_json::to_string(&error.import_from(target)?)?
    ))
}
pub(super) fn msw(client: &GraphqlClient, target: &Path) -> Result<String> {
    let mut out = format!(
        "import {{ graphql, HttpResponse }} from 'msw';\n{}",
        envelope(client, target)?
    );
    for op in &client.definition.operations {
        let symbols = &client.operations[&op.name];
        let kind = if op.kind == GraphqlOperationKind::Query {
            "query"
        } else {
            "mutation"
        };
        writeln!(
            out,
            "export function mock{}(resolve:(variables:{}) => GraphqlMockResponse<{}> | Promise<GraphqlMockResponse<{}>>) {{ return graphql.{kind}({}, async ({{variables}}) => HttpResponse.json(await resolve(variables as unknown as {}))); }}",
            op.name,
            symbols.variables.name,
            symbols.result.name,
            symbols.result.name,
            serde_json::to_string(&op.name)?,
            symbols.variables.name
        )?;
    }
    Ok(out)
}
pub(super) fn cypress(
    client: &GraphqlClient,
    target: &Path,
    options: &CypressOptions,
) -> Result<String> {
    ensure!(
        options.operation_overrides.is_empty(),
        "GraphQL Cypress does not support HTTP-specific operation_overrides"
    );
    let mut out = format!(
        "/// <reference types=\"cypress\" />\n{}export interface GraphqlCypressOptions {{ url:string; headers?:Record<string,string>; timeout?:number; }}\n",
        envelope(client, target)?
    );
    let headers = serde_json::to_string(&options.headers)?;
    for op in &client.definition.operations {
        if op.kind == GraphqlOperationKind::Mutation && !options.include_mutations {
            continue;
        }
        let symbols = &client.operations[&op.name];
        let name = serde_json::to_string(&op.name)?;
        let doc = serde_json::to_string(&op.document)?;
        let default = options
            .base_url
            .as_ref()
            .map(|url| {
                format!(
                    " = {{url:{},headers:{headers},timeout:{}}}",
                    serde_json::to_string(url).unwrap(),
                    options.timeout_ms
                )
            })
            .unwrap_or_default();
        writeln!(
            out,
            "export function request{}(variables:{}, options:GraphqlCypressOptions{default}): Cypress.Chainable<Cypress.Response<GraphqlMockResponse<{}>>> {{ return cy.request<GraphqlMockResponse<{}>>({{method:'POST',url:options.url,headers:{{...{headers},...options.headers}},timeout:options.timeout ?? {},failOnStatusCode:false,body:{{operationName:{name},query:{doc},variables}}}}); }}",
            op.name,
            symbols.variables.name,
            symbols.result.name,
            symbols.result.name,
            options.timeout_ms
        )?;
        writeln!(
            out,
            "export function intercept{}(response:GraphqlMockResponse<{}>, url = '**/graphql') {{ return cy.intercept('POST',url,request => {{ if(request.body && request.body.operationName === {name}) {{ request.alias={name}; request.reply({{statusCode:200,body:response}}); }} else request.continue(); }}); }}",
            op.name, symbols.result.name
        )?;
    }
    Ok(out)
}
