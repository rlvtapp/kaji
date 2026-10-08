import type { NativeAddon } from '@relevate/poolster';

export interface VueQueryPluginOptions { target?: string; output?: string }
export function pluginVueQuery(options?: VueQueryPluginOptions): NativeAddon;
