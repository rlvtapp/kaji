import type { NativePlugin, SdkPackageOptions } from '@relevate/kaji/sdk';

export type RustPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginRust(options?: RustPluginOptions): NativePlugin;
