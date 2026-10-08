import type { NativePlugin, SdkPackageOptions } from '@relevate/kaji/sdk';

export type PythonPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginPython(options?: PythonPluginOptions): NativePlugin;
