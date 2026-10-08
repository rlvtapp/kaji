import type { NativePlugin, SdkPackageOptions } from '@relevate/kaji/sdk';

export type TypeScriptPluginOptions = Omit<SdkPackageOptions, 'language' | 'jobs'>;
export function pluginTypeScript(options?: TypeScriptPluginOptions): NativePlugin;
