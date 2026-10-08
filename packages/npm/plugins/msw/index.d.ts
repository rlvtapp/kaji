import type { NativeAddon } from '@relevate/poolster';

export interface MswPluginOptions { target?: string; output?: string }
export function pluginMsw(options?: MswPluginOptions): NativeAddon;
