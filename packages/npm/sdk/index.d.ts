export interface Operation {
  id: string;
  method: string;
  path: string;
  parameters?: Array<Record<string, unknown>>;
  request_body?: Record<string, unknown>;
  responses?: Array<Record<string, unknown>>;
  security?: Array<Record<string, unknown>>;
  annotations?: Record<string, unknown>;
}

export interface Schema {
  name: string;
  value: Record<string, unknown>;
}

export interface Api {
  name: string;
  version: string;
  operations: Operation[];
  schemas: Schema[];
  annotations: Record<string, unknown>;
}

export interface OutputFile {
  path: string;
  contents: string;
  preserveExisting?: boolean;
  owner?: string;
}

export interface PluginContext {
  readonly api: Api | null;
  readonly input?: InputReport;
  readonly securitySchemes: Record<string, unknown>;
  readonly output: string;
  emitFile(file: Pick<OutputFile, 'path' | 'contents' | 'preserveExisting'>): void;
  readFile(path: string): string | undefined;
  replaceFile(path: string, contents: string): void;
  readonly workspace: Map<string, unknown>;
  readonly inputs: {
    get<T>(contract: JsContract<T>): T;
    optional<T>(contract: JsContract<T>): T | undefined;
  };
  publish<T>(contract: JsContract<T>, value: T): void;
}

export interface JsContract<T = unknown> {
  readonly name: string;
  /** Type-only marker; contract identity is the exported object itself. */
  readonly __value?: T;
}

export interface ProviderHandle<T = unknown> {
  readonly provider: JsPlugin;
  readonly contract: JsContract<T>;
}

export interface ContractRequirement<T = unknown> {
  readonly contract: JsContract<T>;
  readonly from?: ProviderHandle<T>;
  readonly optional: boolean;
}

export interface JsPlugin {
  name: string;
  phase?: 'generate' | 'post';
  requires?: Array<string | ContractRequirement>;
  provides?: JsContract[];
  hooks?: {
    transformApi?: JsPlugin['transformApi'];
    generate?: JsPlugin['generate'];
    schema?: JsPlugin['schema'];
    operation?: JsPlugin['operation'];
  };
  transformApi?(api: Api, context: { securitySchemes: Record<string, unknown>; output: string }): Api | void | Promise<Api | void>;
  generate?(context: PluginContext): void | Promise<void>;
  schema?(schema: Schema, context: PluginContext): void | Promise<void>;
  operation?(operation: Operation, context: PluginContext): void | Promise<void>;
}

export type SdkLanguage = 'typescript' | 'rust' | 'go' | 'python' | 'php' | 'java' | 'csharp' | 'elixir' | 'ruby' | 'swift';

export interface SdkPackageOptions {
  language: SdkLanguage;
  path?: string;
  name?: string;
  version?: string;
  style?: 'flat' | 'namespaced';
  transport?: 'fetch' | 'axios';
  clientName?: string;
  raw?: boolean;
  jobs?: number;
}

export interface NativePlugin {
  kind: 'native-sdk';
  name: string;
  package: SdkPackageOptions & { path: string };
}

export interface NativeAddon {
  kind: 'native-addon';
  name: string;
  plugin: string;
  target: string;
  output?: string;
}

export interface NativeInputPlugin {
  kind: 'native-input';
  name: string;
  format: string;
  provider: string;
}

export interface JsInputPlugin<TData = unknown> {
  kind: 'js-input';
  name: string;
  format: string;
  load(source: string): InputPluginResult<TData> | Promise<InputPluginResult<TData>>;
}

export interface InputPluginResult<TData = unknown> {
  summary: InputReport['summary'];
  diagnostics?: InputReport['diagnostics'];
  data?: TData;
  /** Publishing an HTTP API enables the existing native SDK renderers. */
  api?: Api;
  securitySchemes?: Record<string, unknown>;
}

export interface InputReport {
  provider: string;
  source: string;
  summary: {
    format: string;
    title: string;
    version: string | null;
    types: string[];
    operations: Array<{ name: string; kind: string }>;
  };
  diagnostics: Array<{ code: string; message: string }>;
  data?: unknown;
}

export interface GraphqlScalarMapping { input: string; output: string; }
export interface NativeInputConfig {
  path: string; plugin: NativeInputPlugin | JsInputPlugin;
  /** GraphQL operation files, resolved beside a loaded config file. */
  operations?: string[];
  /** TypeScript wire types; does not perform runtime scalar conversion. */
  scalars?: Record<string, GraphqlScalarMapping>;
  subscriptions?: boolean;
}

export interface PoolsterConfig {
  input: string | { artifacts: string } | NativeInputConfig;
  output: string | { path: string };
  name?: string;
  version?: string;
  compiler?: string;
  plugins: Array<NativePlugin | NativeAddon | JsPlugin>;
}

export interface OutputChanges {
  added: string[];
  modified: string[];
  removed: string[];
}

export interface GenerateResult {
  api: Api | null;
  input?: InputReport;
  files: OutputFile[];
  changes: OutputChanges;
  output: string;
  skipped?: Array<{ path: string; language: string; reason: string }>;
}

export function defineConfig<T extends PoolsterConfig>(config: T): T;
export function definePlugin<TOptions extends unknown[]>(factory: (...options: TOptions) => JsPlugin): (...options: TOptions) => JsPlugin;
export function defineInputPlugin<TOptions extends unknown[], TData = unknown>(factory: (...options: TOptions) => JsInputPlugin<TData>): (...options: TOptions) => JsInputPlugin<TData>;
export function defineContract<T = unknown>(name: string): JsContract<T>;
export function providerHandle<T>(provider: JsPlugin, contract: JsContract<T>): ProviderHandle<T>;
export function requireContract<T>(contract: JsContract<T>, options?: { from?: ProviderHandle<T>; optional?: boolean }): ContractRequirement<T>;
export function availableNativePlugins(): string[];
export function availableInputPlugins(): Array<{ provider: string; format: string }>;
export function inspectInput(input: { path: string; plugin: NativeInputPlugin | JsInputPlugin }): Promise<InputReport>;
export function loadConfig(file?: string): Promise<PoolsterConfig>;
export function generate(config: PoolsterConfig, options?: { write?: boolean }): Promise<GenerateResult>;
export function createPoolster(config: PoolsterConfig): { generate(options?: { write?: boolean }): Promise<GenerateResult>; inspectInput(): Promise<InputReport> };
