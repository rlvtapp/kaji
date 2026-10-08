import type { NativePlugin, SdkPackageOptions } from '@relevate/poolster';

export type TypeScriptPluginOptions = Omit<SdkPackageOptions, 'language' | 'jobs'>;
export function pluginTypeScript(options?: TypeScriptPluginOptions): NativePlugin;
