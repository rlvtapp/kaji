import type { NativePlugin, SdkPackageOptions } from '@relevate/kaji/sdk';

export type JavaPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginJava(options?: JavaPluginOptions): NativePlugin;
