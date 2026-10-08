import type { NativeAddon } from '@relevate/kaji/sdk';

export interface VueQueryPluginOptions { target?: string; output?: string }
export function pluginVueQuery(options?: VueQueryPluginOptions): NativeAddon;
