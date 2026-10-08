import type { NativeAddon } from '@relevate/kaji/sdk';

export interface SwrPluginOptions { target?: string; output?: string }
export function pluginSwr(options?: SwrPluginOptions): NativeAddon;
