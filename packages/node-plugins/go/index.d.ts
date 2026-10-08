import type { NativePlugin, SdkPackageOptions } from '@relevate/kaji/sdk';

export type GoPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw'>;
export function pluginGo(options?: GoPluginOptions): NativePlugin;
