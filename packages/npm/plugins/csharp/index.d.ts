import type { NativePlugin, SdkPackageOptions } from '@relevate/poolster';

export type CSharpPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginCSharp(options?: CSharpPluginOptions): NativePlugin;
