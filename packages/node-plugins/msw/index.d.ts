import type { NativeAddon } from '@relevate/kaji/sdk';

export interface MswPluginOptions { target?: string; output?: string }
export function pluginMsw(options?: MswPluginOptions): NativeAddon;
