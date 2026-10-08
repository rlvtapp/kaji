import type { NativePlugin, SdkPackageOptions } from '@relevate/poolster';

export type PhpPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginPhp(options?: PhpPluginOptions): NativePlugin;
