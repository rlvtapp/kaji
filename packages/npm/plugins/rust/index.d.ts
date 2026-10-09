import type { NativePlugin, SdkPackageOptions } from '@relevate/poolster';

export type RustPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'jobs'>;
export function pluginRust(options?: RustPluginOptions): NativePlugin;
