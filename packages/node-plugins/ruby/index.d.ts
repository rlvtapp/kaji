import type { NativePlugin, SdkPackageOptions } from '@relevate/kaji/sdk';

export type RubyPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginRuby(options?: RubyPluginOptions): NativePlugin;
