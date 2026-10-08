import type { NativePlugin, SdkPackageOptions } from '@relevate/kaji/sdk';

export type CSharpPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginCSharp(options?: CSharpPluginOptions): NativePlugin;
