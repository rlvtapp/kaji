import type { NativePlugin, NativeAddon, NativeInputPlugin, SdkPackageOptions, FixtureOptions, CypressOptions } from '@relevate/poolster';

export type TypeScriptPluginOptions = Omit<SdkPackageOptions, 'language' | 'jobs'>;
export function pluginTypeScript(options?: TypeScriptPluginOptions): NativePlugin;

export type RustPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'jobs'>;
export function pluginRust(options?: RustPluginOptions): NativePlugin;

export type GoPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw'>;
export function pluginGo(options?: GoPluginOptions): NativePlugin;

export type PythonPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginPython(options?: PythonPluginOptions): NativePlugin;

export type PhpPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginPhp(options?: PhpPluginOptions): NativePlugin;

export type JavaPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginJava(options?: JavaPluginOptions): NativePlugin;

export type CSharpPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginCSharp(options?: CSharpPluginOptions): NativePlugin;

export type ElixirPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginElixir(options?: ElixirPluginOptions): NativePlugin;

export type RubyPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginRuby(options?: RubyPluginOptions): NativePlugin;

export type SwiftPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginSwift(options?: SwiftPluginOptions): NativePlugin;

export interface ZodPluginOptions { target?: string; output?: string }
export function pluginZod(options?: ZodPluginOptions): NativeAddon;

export interface FakerPluginOptions { target?: string; output?: string; fixtureOptions?: FixtureOptions }
export function pluginFaker(options?: FakerPluginOptions): NativeAddon;

export interface MswPluginOptions { target?: string; output?: string; fixtureOptions?: FixtureOptions }
export function pluginMsw(options?: MswPluginOptions): NativeAddon;

export interface CypressPluginOptions { target?: string; output?: string; fixtureOptions?: FixtureOptions; cypressOptions?: CypressOptions }
export function pluginCypress(options?: CypressPluginOptions): NativeAddon;

export interface ReactQueryPluginOptions { target?: string; output?: string }
export function pluginReactQuery(options?: ReactQueryPluginOptions): NativeAddon;

export interface VueQueryPluginOptions { target?: string; output?: string }
export function pluginVueQuery(options?: VueQueryPluginOptions): NativeAddon;

export interface SwrPluginOptions { target?: string; output?: string }
export function pluginSwr(options?: SwrPluginOptions): NativeAddon;

export function inputGraphql(): NativeInputPlugin;
export function inputAsyncApi(): NativeInputPlugin;
export function inputArazzo(): NativeInputPlugin;
export function inputProtobuf(): NativeInputPlugin;
export function inputCapnProto(): NativeInputPlugin;
