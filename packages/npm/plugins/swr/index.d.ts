import type { NativeAddon } from '@relevate/poolster';

export interface SwrPluginOptions { target?: string; output?: string }
export function pluginSwr(options?: SwrPluginOptions): NativeAddon;
