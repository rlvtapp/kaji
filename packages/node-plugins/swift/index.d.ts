import type { NativePlugin, SdkPackageOptions } from '@relevate/kaji/sdk';

export type SwiftPluginOptions = Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>;
export function pluginSwift(options?: SwiftPluginOptions): NativePlugin;
