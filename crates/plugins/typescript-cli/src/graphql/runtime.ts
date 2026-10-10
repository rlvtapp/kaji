import { Command } from 'commander';
import { readFile } from 'node:fs/promises';
interface Operation {
  readonly name: string; readonly command: string; readonly kind: 'query' | 'mutation'; readonly document: string;
  readonly required: readonly string[]; readonly nonnull: readonly string[];
}
interface Options { endpoint?: string; variables?: string; variablesFile?: string; header?: string[]; bearerToken?: string; timeout: string; }
const object = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);
export function register(parent: Command, operation: Operation, endpoint: string | null): void {
  parent.command(operation.command).description(`${operation.kind}: ${operation.name}`)
    .option('--endpoint <url>', 'GraphQL HTTP endpoint')
    .option('--variables <json>', 'JSON variables object')
    .option('--variables-file <path>', 'Read JSON variables from a file')
    .option('--header <name:value>', 'HTTP header (repeatable)', (value: string, previous: string[]) => [...previous, value], [])
    .option('--bearer-token <token>', 'Bearer authentication token')
    .option('--timeout <milliseconds>', 'HTTP timeout', '30000')
    .action(async (options: Options) => {
      if (options.variables !== undefined && options.variablesFile !== undefined) throw new Error('Use either --variables or --variables-file');
      const source = options.variablesFile ? await readFile(options.variablesFile, 'utf8') : options.variables ?? '{}';
      let variables: unknown;
      try { variables = JSON.parse(source); } catch { throw new Error('Variables must contain valid JSON'); }
      if (!object(variables)) throw new Error('Variables must be a JSON object');
      for (const name of operation.required) if (!(name in variables)) throw new Error(`Missing required variable: ${name}`);
      for (const name of operation.nonnull) if (name in variables && variables[name] === null) throw new Error(`Variable ${name} cannot be null`);
      const selected = options.endpoint ?? process.env.GRAPHQL_ENDPOINT ?? endpoint;
      if (!selected) throw new Error('Set --endpoint or the endpoint environment variable');
      const url = new URL(selected);
      if (!['http:', 'https:'].includes(url.protocol)) throw new Error('GraphQL endpoint must use HTTP or HTTPS');
      const timeout = Number(options.timeout);
      if (!Number.isSafeInteger(timeout) || timeout <= 0) throw new Error('--timeout must be a positive integer');
      const headers = new Headers({ 'content-type': 'application/json', accept: 'application/graphql-response+json, application/json' });
      for (const header of options.header ?? []) {
        const colon = header.indexOf(':');
        if (colon <= 0) throw new Error('Headers must use name:value');
        headers.set(header.slice(0, colon).trim(), header.slice(colon + 1).trim());
      }
      const token = options.bearerToken ?? process.env.GRAPHQL_TOKEN;
      if (token && !headers.has('authorization')) headers.set('authorization', `Bearer ${token}`);
      try {
        const response = await fetch(url, { method: 'POST', headers, signal: AbortSignal.timeout(timeout), body: JSON.stringify({ query: operation.document, operationName: operation.name, variables }) });
        const value: unknown = await response.json();
        if (!object(value)) throw new Error('Invalid GraphQL response envelope');
        if ('hasNext' in value || 'incremental' in value) throw new Error('GraphQL incremental delivery is unsupported');
        const errors = value.errors;
        if (errors !== undefined && (!Array.isArray(errors) || errors.some(error => !object(error) || typeof error.message !== 'string'))) throw new Error('Invalid GraphQL errors');
        const hasErrors = Array.isArray(errors) && errors.length > 0;
        if (!('data' in value) && !hasErrors) throw new Error('GraphQL response has neither data nor errors');
        if (value.data !== undefined && value.data !== null && !object(value.data)) throw new Error('Invalid GraphQL data');
        if (value.extensions !== undefined && !object(value.extensions)) throw new Error('Invalid GraphQL extensions');
        if (!response.ok && !hasErrors) throw new Error(`GraphQL HTTP request failed (${response.status})`);
        console.log(JSON.stringify(value));
        if (hasErrors) {
          console.error('GraphQL response contains errors');
          process.exitCode = value.data !== undefined && value.data !== null ? 3 : 4;
        }
      } catch (error) {
        console.error(error instanceof Error ? error.message : String(error));
        process.exitCode = 2;
      }
    });
}
