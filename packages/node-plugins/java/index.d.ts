import type { NativePlugin, SdkPackageOptions } from '@relevate/poolster';

export type JavaPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginJava(options?: JavaPluginOptions): NativePlugin;
