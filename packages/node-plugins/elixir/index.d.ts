import type { NativePlugin, SdkPackageOptions } from '@relevate/poolster';

export type ElixirPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginElixir(options?: ElixirPluginOptions): NativePlugin;
