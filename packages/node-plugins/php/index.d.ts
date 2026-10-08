import type { NativePlugin, SdkPackageOptions } from '@relevate/kaji/sdk';

export type PhpPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginPhp(options?: PhpPluginOptions): NativePlugin;
